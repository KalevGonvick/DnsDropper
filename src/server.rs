mod dns;
mod server_config;
mod filter;
mod logging;
mod internal;

use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::env;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use env_logger::fmt::style::Style;
use tokio::io::Result;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::filter::Filter;
use crate::dns::{BytePacketBuffer, DnsPacket, ResultCode};
use crate::internal::INTERNAL_CONFIG;
use crate::logging::{GetStyle, HighlightStyle};
use crate::logging::HighlightStyle::ErrorHighlight;
use crate::server_config::ServerConfig;

fn main() -> Result<()> {
    let mut args: VecDeque<String> = env::args().collect();
    let mut config_dir: String = INTERNAL_CONFIG.default_server_config_dir.to_string();
    if args.len() > 1 {
        args.pop_front();
        while !args.is_empty() {
            let arg_key: String = args.pop_front().expect("");
            let arg_value: String = args.pop_front().expect("");
            if arg_key.eq("--config") || arg_key.eq("-c") {
                config_dir = arg_value;
            }
        }
    }



    match ServerConfig::load_from(std::path::Path::new(&config_dir)) {
        Ok(server_config) => {
            logging::setup(server_config.logging.level.as_ref());
            logging::print_title();
            start_server(&&server_config).expect("Failed to start server");
            Ok(())
        }

        Err(_) => {
            Err(Error::new(ErrorKind::InvalidInput, std::format!("Failed to read server.yaml from the provided path: {}", config_dir)))
        }
    }
}


fn start_server<'a>(config: &'a Cow<'a, ServerConfig>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.server.worker_thread_count as usize)
        .thread_name_fn(||{
            static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
            let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
            format!("{}-{}",INTERNAL_CONFIG.worker_thread_name, id)
        })
        .enable_io()
        .enable_time()
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

        /* Lots of traffic produced when using loopback configuration on Windows. Why? I have no idea and I don't look forward to knowing. */
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
                    let mut response_buffer = BytePacketBuffer::new();
                    let style: Style = HighlightStyle::get_style(ErrorHighlight);

                    let num = query.record_type.to_num();
                    log::trace!("{style}BLOCK{style:#}: {}:{}", query.name, num);

                    packet.header.result_code = ResultCode::NXDOMAIN;
                    match packet.write(&mut response_buffer) {
                        Ok(_) => {
                            match response_buffer.get_range(0, response_buffer.pos) {
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

    log::trace!("UDP socket bound to {:?} for: {:?}", socket.local_addr(), remote_host);

    let data: Result<Vec<u8>> = timeout(duration, async {
        socket.send_to(buf, remote_host.to_string()).await?;
        let mut response = [0; INTERNAL_CONFIG.max_udp_packet_size as usize];
        let length = socket.recv(&mut response).await?;
        Ok(response[..length].to_vec())
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
