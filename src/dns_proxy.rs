
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::{Error, ErrorKind};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use crate::dns::{BytePacketBuffer, DnsPacket, ResultCode};
use log::{error, trace};
use crate::filter::Filter;
use crate::{filter, logging};
use crate::logging::HighlightStyle::{DebugHighlight, ErrorHighlight};

#[derive(Debug)]
pub(crate) struct DnsProxy {
    pub(crate) complete_block_list: HashSet<Filter>
}

impl DnsProxy {

    async fn do_lookup(&self, buf: &[u8], remote_host: String, connection_timeout: i64) -> std::io::Result<Vec<u8>> {
        let duration = Duration::from_millis(connection_timeout as u64);
        let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;

        trace!("Creating outbound UDP socket for: {:?}", remote_host);

        // TODO - is this the best way to do this?
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

    pub(crate) async fn dns_lookup(&self, mut req: BytePacketBuffer, len: usize, remote_hosts: &[Cow<'_, str>], connection_timeout: i64) -> std::io::Result<Vec<u8>> {
        let mut request = DnsPacket::from_buffer(&mut req)?;
        let query = request.questions.get(0).unwrap();

        // TODO - probably move this filtering out of here.
        if filter::should_filter(query.name.clone(), &self.complete_block_list) {
            let style = logging::get_highlight_style(ErrorHighlight);
            trace!("{style}BLOCK{style:#}: {:?}:{:?}", query.name, query.record_type);

            // TODO - Handle responding in a better way than just NXDOMAIN
            // TODO - Also caching would be nice so we don't need to calculate every time
            request.header.result_code = ResultCode::NXDOMAIN;
            let mut res_buffer = BytePacketBuffer::new();
            request.write(&mut res_buffer)?;
            let data = res_buffer.get_range(0, res_buffer.pos())?;
            Ok(data.to_vec())

        } else {
            let style = logging::get_highlight_style(DebugHighlight);
            trace!("{style}ALLOW{style:#}: {:?}:{:?}", query.name, query.record_type);
            return self.do_lookup(&req.buf[..len], remote_hosts.get(0).unwrap().to_string(), connection_timeout).await;
        }
    }
}

