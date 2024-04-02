mod dns;
mod server_config;
mod filter;
mod logging;
mod internal;


use std::borrow::Cow;
use std::collections::{HashSet};
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::Result;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use clap::Parser;
use tokio::runtime::Runtime;

use crate::filter::{Filter, FilterList};
use crate::logging::WARN_STYLE;
use crate::logging::ERROR_STYLE;
use crate::dns::{BytePacketBuffer, DnsPacket, DnsQuestion, ResultCode};
use crate::internal::INTERNAL_CONFIG;
use crate::server_config::ServerConfig;

fn main() -> Result<()> {
    let args = ClArgs::parse();
    let mut config_dir: String = INTERNAL_CONFIG.default_server_config_dir.to_string();
    let config_args = args.config;

    if !config_args.is_empty() {
        config_dir = config_args;
    }

    match ServerConfig::load_from(std::path::Path::new(&config_dir)) {
        Ok(server_config) => {
            logging::setup(server_config.logging.level.as_ref());
            logging::print_title();
            let server: Server = Server::new(server_config);
            server.start()
        }

        Err(_) => {
            Err(Error::new(ErrorKind::InvalidInput, std::format!("Failed to read server.yaml from the provided path: {}", config_dir)))
        }
    }
}

struct Server {
    runtime: Runtime,
    server_config: Arc<ServerConfig>
}

impl Server {
    pub fn new(
        config: Cow<ServerConfig>
    ) -> Server {
        let arc_config = Arc::new(config.clone().into_owned());
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
            .build();

        Server {
            server_config: arc_config,
            runtime: rt.unwrap()
        }
    }

    pub fn start(
        &self
    ) -> Result<()> {

        self.runtime.block_on(async {

            let filter_list: FilterList = FilterList::new(self.server_config.clone().udp_proxy.domain_block_lists.as_ref());
            let complete_block_list = filter_list.resolved_block_list().await;
            let bind = self.server_config.udp_proxy.bind.to_string();

            let socket = UdpSocket::bind(bind).await?;
            let arc_socket = Arc::new(socket);

            log::debug!("Block list contains '{}' different domain names.", complete_block_list.len());

            let arc_block_list = Arc::new(complete_block_list);
            log::info!("Started DNS Proxy: {}", self.server_config.udp_proxy.bind);
            loop {
                match arc_socket.ready(tokio::io::Interest::READABLE).await {
                    Ok(r) => {
                        if r.is_readable() {

                            /* shared constants for threads */
                            let arc_socket_clone = arc_socket.clone();
                            let arc_config_clone = self.server_config.clone();
                            let arc_block_list_clone = arc_block_list.clone();

                            tokio::spawn(async move {
                                let server_thread = ServerThread::new(
                                    arc_socket_clone,
                                    arc_config_clone,
                                    arc_block_list_clone
                                );
                                server_thread.start().await
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
}

struct ServerThread {
    socket: Arc<UdpSocket>,
    server_config: Arc<ServerConfig>,
    block_list: Arc<HashSet<Filter>>
}

impl ServerThread {
    pub fn new(
        socket: Arc<UdpSocket>,
        server_config: Arc<ServerConfig>,
        block_list: Arc<HashSet<Filter>>
    ) -> ServerThread {
        ServerThread {
            socket,
            server_config,
            block_list
        }
    }

    pub async fn start(
        &self
    ) {

        let mut req = BytePacketBuffer::new();
        let (len, src) = match self.socket.try_recv_from(&mut req.buf) {

            Ok(r) => {
                log::trace!("DNS Listener received data of length: {} bytes", r.0);
                r
            },

            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                return;
            }

            /* Lots of traffic produced when using loopback configuration on Windows. Why? I have no idea and I don't look forward to knowing. */
            Err(ref e) if e.kind() == ErrorKind::ConnectionReset => {
                log::trace!("Connection reset error occurred for local socket '{}'.", self.socket.local_addr().unwrap());
                return;
            }

            Err(err) => {
                log::error!("Failed to receive message from configured bind: {}", err);
                return;
            }
        };

        if let Ok(mut packet) = DnsPacket::from_buffer(&mut req) {
            log::trace!("Successfully constructed a DNS packet from received data.");
            match packet.questions.get(0) {

                None => {
                    log::error!("DNS Packet contains no questions.");
                }

                Some(query) => {
                    log::trace!("Checking to see if the domain '{}' and/or record type '{}' should be filtered.", query.name, query.record_type.to_num());

                    // TODO - maybe introduce handlers?
                    if filter::should_filter(&query.name, &self.block_list) {
                        let mut response_buffer = BytePacketBuffer::new();
                        log::info!("{ERROR_STYLE}!BLOCK!{ERROR_STYLE:#} -- {WARN_STYLE}{}:{}{WARN_STYLE:#}", query.name, query.record_type.to_num());
                        packet.header.result_code = ResultCode::NXDOMAIN;

                        match packet.write(&mut response_buffer) {
                            Ok(_) => {
                                match response_buffer.get_range(0, response_buffer.pos) {
                                    Ok(data) => {
                                        if let Err(err) = self.socket.send_to(&data, &src).await {
                                            log::error!("Reply to '{}' failed {:?}.", &src, err);
                                        }
                                    }
                                    Err(err) => {
                                        log::error!("Could not retrieve buffer range: {}.", err);
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!("Error writing packet: {}.", err);
                            }
                        }

                    } else {
                        self.handle_proxy(req, len, src, query).await
                    }
                }
            }
        }
    }

    fn handle_filtered_response() {

    }

    async fn handle_proxy(
        &self,
        req: BytePacketBuffer,
        len: usize,
        src: SocketAddr,
        query: &DnsQuestion
    ) {
        for host in self.server_config.udp_proxy.dns_hosts.as_ref() {

            match self.do_proxy(&req.buf[..len], host.to_string()).await {
                Ok(data) => {
                    if let Err(err) = self.socket.send_to(&data, &src).await {
                        log::error!("Replying to '{}' failed {:?}.", &src, err);
                        continue;
                    }
                    log::debug!("Forwarded the answer for the domain '{}' from '{}'.", query.name, host);
                    return;
                }
                Err(err) => {
                    log::error!("Error processing request: {:?}.", err);
                    return;
                }
            }
        }
    }

    async fn do_proxy(
        &self,
        buf: &[u8],
        remote_host: String
    ) -> Result<Vec<u8>> {
        let duration = Duration::from_millis(self.server_config.udp_proxy.timeout as u64);
        let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;

        log::debug!("Outbound UDP socket bound to port '{}' for the host destination '{}'.", socket.local_addr().unwrap().port(), remote_host);

        let data: Result<Vec<u8>> = timeout(duration, async {
            socket.send_to(buf, remote_host.to_string()).await?;
            let mut response = [0; INTERNAL_CONFIG.max_udp_packet_size as usize];
            let length = socket.recv(&mut response).await?;
            log::debug!("Received response from '{}' for port '{}' with a length of: {} bytes", remote_host, socket.local_addr().unwrap().port(), length);
            Ok(response[..length].to_vec())
        }).await?;

        match data {
            Ok(data) => {
                return Ok(data)
            }
            Err(err) => {
                log::error!("Agent request to {:?} {:?}", remote_host, err);
                Err(err)
            }
        }
    }
}

// A customizable light-weight DNS proxy with domain filtering capabilities.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct ClArgs {

    // Path to the config file.
    #[arg(short, long)]
    config: String
}
