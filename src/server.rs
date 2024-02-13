mod dns;
mod server_config;
mod filter;
mod logging;

use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::Result;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::filter::Filter;
use crate::dns::{BytePacketBuffer, DnsPacket, ResultCode};
use crate::logging::HighlightStyle::ErrorHighlight;
use crate::server_config::ServerConfig;

fn main() -> Result<()> {
    match ServerConfig::load_from(std::path::Path::new("config/server.yaml")) {
        Ok(server_config) => {
            logging::setup(server_config.logging.level.as_ref());
            logging::print_title();
            start_server(&&server_config).expect("Failed to start server");
        }

        Err(err) => {
            println!("Failed to read server.yaml: {}", err);
        }
    };

    Ok(())
}


fn start_server<'a>(config: &'a Cow<'a, ServerConfig>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.server.worker_thread_count as usize)
        .thread_name("WORKER")
        .enable_all()
        .build()?;

    rt.block_on(async {
        let complete_block_list = filter::load_block_list(config.clone().udp_proxy.domain_block_lists.as_ref()).await;
        let bind = config.udp_proxy.bind.to_string();

        log::info!("Creating server listening on bind: {}", bind);

        let socket = UdpSocket::bind(bind).await?;
        let arc_socket = Arc::new(socket);
        let arc_config = Arc::new(config.clone().into_owned());

        log::debug!("Block list contains '{}' different domain names.", complete_block_list.len());

        let arc_block_list = Arc::new(complete_block_list);

        loop {
            match arc_socket.ready(tokio::io::Interest::READABLE).await {
                Ok(r) => {
                    if r.is_readable() {

                        /* shared constants for threads */
                        let arc_socket_clone = arc_socket.clone();
                        let arc_config_clone = arc_config.clone();
                        let arc_block_list_clone = arc_block_list.clone();
                        tokio::spawn(async move {
                            start_udp_dns_listener(arc_socket_clone, arc_config_clone, arc_block_list_clone).await;
                        });
                    }
                }
                Err(err) => {
                    log::error!("Error trying to read from socket: {}", err);
                }
            }
        }
    })
}

async fn start_udp_dns_listener(socket: Arc<UdpSocket>, server_config: Arc<ServerConfig>, block_list: Arc<HashSet<Filter>>) {
    let mut req = BytePacketBuffer::new();
    let (len, src) = match socket.try_recv_from(&mut req.buf) {
        Ok(r) => r,
        Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
            return;
        }

        /* Lots of traffic produced when used on Windows. Why? I have no idea, I don't look forward to knowing.*/
        Err(ref e) if e.kind() == ErrorKind::ConnectionReset => {
            return;
        }

        Err(err) => {
            log::error!("Failed to receive message from configured bind: {:?}", err.kind());
            return;
        }
    };

    if let Ok(mut packet) = DnsPacket::from_buffer(&mut req) {
        match packet.questions.get(0) {
            None => {
                log::error!("Packet contains no questions");
            }
            Some(query) => {
                if filter::should_filter(&query.name, &block_list) {
                    let mut res_buffer = BytePacketBuffer::new();
                    let style = logging::get_highlight_style(ErrorHighlight);

                    log::trace!("{style}BLOCK{style:#}: {:?}:{:?}", query.name, query.record_type);

                    packet.header.result_code = ResultCode::NXDOMAIN;
                    match packet.write(&mut res_buffer) {
                        Ok(_) => {
                            match res_buffer.get_range(0, res_buffer.pos) {
                                Ok(data) => {
                                    if let Err(err) = socket.send_to(&data, &src).await {
                                        log::error!("Reply to '{}' failed {:?}", &src, err);
                                    }
                                }
                                Err(err) => {
                                    log::error!("Could not retrieve buffer range: {}", err);
                                }
                            }
                        }
                        Err(err) => {
                            log::error!("Error writing packet: {}", err);
                        }
                    }
                } else {
                    for host in server_config.udp_proxy.dns_hosts.iter() {
                        match do_lookup(&req.buf[..len], host.to_string(), server_config.udp_proxy.timeout).await {
                            Ok(data) => {
                                if let Err(err) = socket.send_to(&data, &src).await {
                                    log::error!("Replying to '{}' failed {:?}", &src, err);
                                    continue;
                                }
                                return;
                            }
                            Err(err) => {
                                log::error!("Error processing request: {:?}", err);
                            }
                        };
                    }
                }
            }
        }
    }
}

async fn do_lookup(buf: &[u8], remote_host: String, connection_timeout: i64) -> Result<Vec<u8>> {
    let duration = Duration::from_millis(connection_timeout as u64);
    let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;

    let mut hasher = DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    log::trace!("[{:?}]Creating outbound UDP socket for: {:?}", hasher.finish(), remote_host);

    let data: Result<Vec<u8>> = timeout(duration, async {
        socket.send_to(buf, remote_host.to_string()).await?;
        let mut res = [0; 4096];
        let len = socket.recv(&mut res).await?;
        Ok(res[..len].to_vec())
    }).await?;

    match data {
        Ok(data) => {
            return Ok(data);
        }
        Err(err) => {
            log::error!("Agent request to {:?} {:?}", remote_host, err);
        }
    }
    Err(Error::new(ErrorKind::Other, "Proxy server failed to proxy request"))
}
