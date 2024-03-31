mod dns;
mod server_config;
mod filter;
mod logging;
mod internal;

use crate::logging::WARN_STYLE;
use crate::logging::ERROR_STYLE;
use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::env;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::Result;
use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::filter::Filter;
use crate::dns::{BytePacketBuffer, DnsPacket, ResultCode};
use crate::internal::INTERNAL_CONFIG;
use crate::server_config::ServerConfig;

fn main() -> Result<()> {
    let mut args: VecDeque<String> = env::args().collect();
    let mut config_dir: String = INTERNAL_CONFIG.default_server_config_dir.to_string();

    // TODO - there is probably a crate for this already. use that.
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

            start_server(server_config)
        }

        Err(_) => {
            Err(Error::new(ErrorKind::InvalidInput, std::format!("Failed to read server.yaml from the provided path: {}", config_dir)))
        }
    }
}


fn start_server<'a>(
    config: Cow<ServerConfig>
) -> Result<()> {

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.server.worker_thread_count as usize)
        .thread_name_fn(|| {
            static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
            let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
            format!("{}-{}", INTERNAL_CONFIG.worker_thread_name, id)
        })
        .on_thread_start(|| {
            log::debug!("Starting worker thread.");
        })
        .on_thread_stop(|| {
            log::debug!("Stopping worker thread.");
        })
        .enable_io()
        .enable_time()
        .build()?;

    rt.block_on(async {
        let complete_block_list = filter::load_filtered_domains(config.clone().udp_proxy.domain_block_lists.as_ref()).await;
        let bind = config.udp_proxy.bind.to_string();

        let socket = UdpSocket::bind(bind).await?;
        let arc_socket = Arc::new(socket);
        let arc_config = Arc::new(config.clone().into_owned());

        log::debug!("Block list contains '{}' different domain names.", complete_block_list.len());

        let arc_block_list = Arc::new(complete_block_list);
        log::info!("Started DNS Proxy: {}", config.udp_proxy.bind);
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

async fn start_udp_dns_listener(
    socket: Arc<UdpSocket>,
    server_config: Arc<ServerConfig>,
    block_list: Arc<HashSet<Filter>>
) {
    let mut req = BytePacketBuffer::new();
    let (len, src) = match socket.try_recv_from(&mut req.buf) {
        Ok(r) => {
            log::trace!("DNS Listener received data of length: {} bytes", r.0);
            r
        },

        Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
            return;
        }

        /* Lots of traffic produced when using loopback configuration on Windows. Why? I have no idea and I don't look forward to knowing. */
        Err(ref e) if e.kind() == ErrorKind::ConnectionReset => {
            log::trace!("Connection reset error occurred for local socket '{}'.", socket.local_addr().unwrap());
            return;
        }

        Err(err) => {
            log::error!("Failed to receive message from configured bind: {}", err);
            return;
        }
    };

    if let Ok(mut packet) = DnsPacket::from_buffer(&mut req) {
        log::trace!("Successfully constructed a DNS packet from received data.");

        if let None = packet.questions.get(0) {
            log::error!("DNS Packet contains no questions");

        } else if let Some(query) = packet.questions.get(0) {
            log::trace!("Checking to see if the domain '{}' and/or record type '{}' should be filtered.", query.name, query.record_type.to_num());

            if filter::should_filter(&query.name, &block_list) {
                let mut response_buffer = BytePacketBuffer::new();
                log::info!("{ERROR_STYLE}!BLOCK!{ERROR_STYLE:#} -- {WARN_STYLE}{}:{}{WARN_STYLE:#}", query.name, query.record_type.to_num());
                packet.header.result_code = ResultCode::NXDOMAIN;

                if let Ok(_) = packet.write(&mut response_buffer) {

                    if let Ok(data) = response_buffer.get_range(0, response_buffer.pos) {

                        if let Err(err) = socket.send_to(&data, &src).await {
                            log::error!("Reply to '{}' failed {:?}", &src, err);
                        }

                    } else if let Err(err) = response_buffer.get_range(0, response_buffer.pos) {
                        log::error!("Could not retrieve buffer range: {}", err);
                    }

                } else if let Err(err) = packet.write(&mut response_buffer) {
                    log::error!("Error writing packet: {}", err);
                }

            } else {

                for host in server_config.udp_proxy.dns_hosts.as_ref() {

                    if let Ok(data) = do_lookup(&req.buf[..len], host.to_string(), server_config.udp_proxy.timeout).await {

                        if let Err(err) = socket.send_to(&data, &src).await {
                            log::error!("Replying to '{}' failed {:?}", &src, err);
                            continue;
                        }

                        log::debug!("Forwarded the answer for the domain '{}' from '{}'.", query.name, host);
                        return;

                    } else if let Err(err) = do_lookup(&req.buf[..len], host.to_string(), server_config.udp_proxy.timeout).await {
                        log::error!("Error processing request: {:?}", err);
                        return;

                    } else {
                        unreachable!()
                    };
                }
            }
        }
    }
}

async fn do_lookup(buf: &[u8], remote_host: String, connection_timeout: i64) -> Result<Vec<u8>> {
    let duration = Duration::from_millis(connection_timeout as u64);
    let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;

    log::debug!("Outbound UDP socket bound to port '{}' for the host destination '{}'.", socket.local_addr().unwrap().port(), remote_host);

    let data: Result<Vec<u8>> = timeout(duration, async {
        socket.send_to(buf, remote_host.to_string()).await?;

        let mut response = [0; INTERNAL_CONFIG.max_udp_packet_size as usize];
        let length = socket.recv(&mut response).await?;

        log::debug!("Received response from '{}' for port '{}' with a length of: {} bytes", remote_host, socket.local_addr().unwrap().port(), length);

        Ok(response[..length].to_vec())
    }).await?;

    if let Ok(data) = data {
        return Ok(data)

    } else if let Err(err) = data {
        log::error!("Agent request to {:?} {:?}", remote_host, err);
        Err(err)

    } else {
        unreachable!()
    }
}
