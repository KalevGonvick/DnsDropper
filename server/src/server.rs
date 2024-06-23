use std::borrow::Cow;
use logger::WARN_STYLE;
use logger::ERROR_STYLE;
use std::collections::HashSet;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use log::Level;
use tokio::net::UdpSocket;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::time::timeout;
use packet::{BytePacketBuffer};
use packet::dns::{DnsPacket, DnsQuestion, ResultCode};

use crate::config::internal::INTERNAL_CONFIG;
use crate::config::server_config::ServerConfig;
use crate::exchange::{Exchange, ExchangeState};
use crate::packet_handler::PacketHandler;
use crate::proxy_handler::ProxyHandler;

pub mod config {
    pub mod internal;
    pub mod server_config;
}
//mod filter;
mod packet_handler;
mod filter_handler;
mod proxy_handler;
mod exchange;

pub struct Server {
    runtime: Runtime,
    server_config: Arc<ServerConfig>
}

impl Server {
    pub fn new(config: Cow<ServerConfig>) -> Server {
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
            .build().expect("Failed to create new runtime!");

        Server {
            server_config: arc_config,
            runtime: rt
        }
    }

    pub fn start(&self) -> std::io::Result<()> {
        self.runtime.block_on(async {
            let handler_list = match self.resolve_handlers() {
                Ok(handlers) => {
                    handlers
                }
                Err(_) => {
                    return Err(Error::new(ErrorKind::InvalidInput, "Failed to load handlers."));
                }
            };
            let bind = self.server_config.udp_proxy.bind.to_string();
            let socket = UdpSocket::bind(bind).await?;
            let arc_socket = Arc::new(socket);
            let arc_handlers = Arc::new(Mutex::new(handler_list));
            log::info!("Started DNS Proxy: {}", self.server_config.udp_proxy.bind);
            loop {
                match arc_socket.ready(tokio::io::Interest::READABLE).await {
                    Ok(r) => {
                        if r.is_readable() {

                            /* shared constants for threads */
                            let arc_socket_clone = arc_socket.clone();
                            let arc_config_clone = self.server_config.clone();
                            let arc_handlers_clone = arc_handlers.clone();

                            tokio::spawn(async {
                                let mut req = BytePacketBuffer::new(4096);
                                let (len, src) = match arc_socket_clone.try_recv_from(&mut req.buf) {
                                    Ok(r) => {
                                        log::trace!("DNS Listener received data of length: {} bytes", r.0);
                                        r
                                    },

                                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                                        return;
                                    }

                                    /* Lots of traffic produced when using loopback configuration on Windows. Why? I have no idea and I don't look forward to knowing. */
                                    Err(ref e) if e.kind() == ErrorKind::ConnectionReset => {
                                        log::trace!("Connection reset error occurred for local socket '{}'.", arc_socket_clone.local_addr().unwrap());
                                        return;
                                    }

                                    Err(err) => {
                                        log::error!("Failed to receive message from configured bind: {}", err);
                                        return;
                                    }
                                };
                                let exchange: Exchange = Exchange::new(req, len, src, arc_socket_clone, arc_config_clone);
                                let mut executor = HandlerExecutor::new(
                                    exchange,
                                    arc_handlers_clone
                                );
                                executor.execute_handlers().await
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


    fn resolve_handlers(&self) -> Result<Vec<Box<dyn PacketHandler + Send + Sync>>, ()> {
        let proxy_handler = Box::new(ProxyHandler {}) as Box<dyn PacketHandler + Send + Sync>;
        Ok(vec![proxy_handler])
    }
}

pub struct HandlerExecutor {
    exchange: Exchange,
    handlers: Arc<Mutex<Vec<Box<dyn PacketHandler + Send + Sync>>>>
}

impl HandlerExecutor {
    pub fn new(exchange: Exchange, handlers: Arc<Mutex<Vec<Box<dyn PacketHandler + Send + Sync>>>>) -> HandlerExecutor {
        HandlerExecutor {
            exchange,
            handlers
        }
    }


    pub async fn execute_handlers(&mut self) {
        let locked_handlers = self.handlers.lock().await;
        for handler in locked_handlers.iter() {
            log::info!("HANDLER_START");
            logger::log_type(handler, Level::Info);
            handler.exec(&mut self.exchange);
            log::info!("HANDLER_END");
            match self.exchange.get_state() {
                ExchangeState::INITIAL => {
                    log::info!("Exchange ended with initial");
                    continue;
                }
                ExchangeState::OK => {
                    log::info!("Exchange ended with ok");
                    continue;
                }
                ExchangeState::COMPLETE => {
                    log::info!("Exchange ended with complete");
                    return;
                }
                ExchangeState::FAILED => {
                    log::info!("Exchange ended with failed");
                    return;
                }
                ExchangeState::INVALID => {
                    log::info!("Exchange ended with invalid");
                    return;
                }
            }

        }

    }

}
