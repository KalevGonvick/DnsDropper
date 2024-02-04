use std::borrow::Cow;
use std::collections::HashSet;
use std::io::{Error, ErrorKind};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use crate::dns::{BytePacketBuffer, DnsPacket, DnsQuestion, RecordType, ResultCode};
use log::{debug, error, info, trace};
use crate::filter::Filter;
use crate::logging;
use crate::logging::HighlightStyle::{DebugHighlight, DefaultHighlight, ErrorHighlight};

#[derive(Debug)]
pub(crate) struct DnsProxy {
    pub(crate) complete_block_list: HashSet<Filter>
}

impl DnsProxy {

    async fn proxy(&self, buf: &[u8], remote_host: String, connection_timeout: i64) -> std::io::Result<Vec<u8>> {
        let duration = Duration::from_millis(connection_timeout as u64);
        let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;

        // TODO - I think the first remote_host ever gets used here

        trace!("Creating outbound UDP socket for: {:?}", remote_host);


        let data: std::io::Result<Vec<u8>> = timeout(duration, async {
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
                error!("Agent request to {:?} {:?}", remote_host, err);
            }
        }


        Err(Error::new(ErrorKind::Other, "Proxy server failed to proxy request", ))
    }


    fn should_filter(&self, domain: String) -> bool {
        let style = logging::get_highlight_style(DefaultHighlight);
        for entry in &self.complete_block_list {
            if entry.domain == domain {
                info!("Block-List contains the name '{style}{}{style:#}'", domain);
                return true;
            }
        }
        return false;
    }

    pub(crate) async fn handle(&self, mut req: BytePacketBuffer, len: usize, remote_hosts: &[Cow<'_, str>], connection_timeout: i64) -> std::io::Result<Vec<u8>> {
        let mut request = DnsPacket::from_buffer(&mut req)?;

        // {
        //     Some(q) => q,
        //     None => DnsQuestion::new("".to_string(), RecordType::UNKNOWN(0)) //self.proxy(&req.buf[..len], remote_hosts.get(0).unwrap().to_string(), connection_timeout).await,
        // };


        let query = request.questions.get(0).unwrap();

        let should_filter = self.should_filter(query.name.clone());

        if should_filter {
            let style = logging::get_highlight_style(ErrorHighlight);
            trace!("{style}BLOCK{style:#}: {:?}:{:?}", query.name, query.record_type);
            request.header.result_code = ResultCode::NXDOMAIN;
            let mut res_buffer = BytePacketBuffer::new();
            request.write(&mut res_buffer)?;
            let data = res_buffer.get_range(0, res_buffer.pos())?;
            Ok(data.to_vec())

        } else {
            let style = logging::get_highlight_style(DebugHighlight);
            trace!("{style}ALLOW{style:#}: {:?}:{:?}", query.name, query.record_type);
            return self.proxy(&req.buf[..len], remote_hosts.get(0).unwrap().to_string(), connection_timeout).await;
        }


    }
}