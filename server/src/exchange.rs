use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use packet::BytePacketBuffer;
use crate::config::server_config::ServerConfig;

pub enum ExchangeState {
    INITIAL,
    OK,
    COMPLETE,
    FAILED,
    INVALID
}

pub struct Exchange {
    pub buffer: BytePacketBuffer,
    pub server_config: Arc<ServerConfig>,
    pub socket: Arc<UdpSocket>,
    pub len: usize,
    pub src: SocketAddr,
    pub state: ExchangeState
}

impl Exchange {
    pub fn new(buffer: BytePacketBuffer, len: usize, src: SocketAddr, socket: Arc<UdpSocket>, server_config: Arc<ServerConfig>) -> Exchange {
        Exchange {
            buffer,
            server_config,
            socket,
            len,
            src,
            state: ExchangeState::INITIAL
        }
    }

    pub fn get_state(&self) -> &ExchangeState {
        &self.state
    }
}