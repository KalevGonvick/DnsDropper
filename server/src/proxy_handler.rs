use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use packet::BytePacketBuffer;
use crate::config::internal::INTERNAL_CONFIG;
use crate::config::server_config::ServerConfig;
use crate::exchange;
use crate::exchange::{Exchange, ExchangeState};
use crate::packet_handler::PacketHandler;

pub struct ProxyHandler;

impl PacketHandler for ProxyHandler {
    fn exec(&self, exchange: &mut Exchange) {
        for host in exchange.server_config.udp_proxy.dns_hosts.as_ref() {
            futures::executor::block_on(async {
                match self.do_proxy(&exchange.buffer.buf[..exchange.len], host.to_string(), exchange.server_config.udp_proxy.timeout as u64).await {

                    Ok(data) => {

                        if let Err(err) = exchange.socket.send_to(&data, &exchange.src).await {
                            log::error!("Replying to '{}' failed {:?}.", &exchange.src, err);
                            exchange.state = ExchangeState::FAILED;

                        } else {
                            match exchange.state {
                                ExchangeState::INITIAL => {
                                    exchange.state = ExchangeState::OK;
                                }
                                | _ => {}
                            }
                            return;
                        }
                    }

                    Err(err) => {
                        log::error!("Error processing request: {:?}.", err);
                        exchange.state = ExchangeState::INVALID;
                    }
                }
            });
        }
    }
}

impl ProxyHandler {
    async fn do_proxy(&self, buf: &[u8], remote_host: String, timeout_duration: u64) -> std::io::Result<Vec<u8>> {
        let duration = Duration::from_millis(timeout_duration);
        let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;

        log::debug!("Outbound UDP socket bound to port '{}' for the host destination '{}'.", socket.local_addr().unwrap().port(), remote_host);

        let data: std::io::Result<Vec<u8>> = timeout(duration, async {
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



