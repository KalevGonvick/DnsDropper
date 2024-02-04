mod dns;
mod dns_proxy;
mod server_config;
mod filter;
mod logging;

use std::{fs, io};
use std::borrow::Cow;
use std::collections::HashSet;

use std::io::{BufReader, Read, Write};
use std::sync::Arc;
use env_logger::fmt::style::{Ansi256Color, Color, Style};
use log::{debug, error, info, trace};
use reqwest::StatusCode;
use tokio::{io::{Result}};
use tokio::io::Interest;
use tokio::net::UdpSocket;
use tokio::runtime::{Builder};
use crate::filter::Filter;
use url::{Url};

use crate::dns::BytePacketBuffer;
use crate::dns_proxy::DnsProxy;
use crate::server_config::ServerConfig;

fn main() -> Result<()> {
    let config = ServerConfig::load();
    logging::setup(config.logging.level.as_ref());
    let title_style = Style::new().fg_color(Some(Color::Ansi256(Ansi256Color(126))));
    let art = r#"
████████▄  ███▄▄▄▄      ▄████████      ████████▄     ▄████████  ▄██████▄     ▄███████▄    ▄███████▄    ▄████████    ▄████████
███   ▀███ ███▀▀▀██▄   ███    ███      ███   ▀███   ███    ███ ███    ███   ███    ███   ███    ███   ███    ███   ███    ███
███    ███ ███   ███   ███    █▀       ███    ███   ███    ███ ███    ███   ███    ███   ███    ███   ███    █▀    ███    ███
███    ███ ███   ███   ███             ███    ███  ▄███▄▄▄▄██▀ ███    ███   ███    ███   ███    ███  ▄███▄▄▄      ▄███▄▄▄▄██▀
███    ███ ███   ███ ▀███████████      ███    ███ ▀▀███▀▀▀▀▀   ███    ███ ▀█████████▀  ▀█████████▀  ▀▀███▀▀▀     ▀▀███▀▀▀▀▀
███    ███ ███   ███          ███      ███    ███ ▀███████████ ███    ███   ███          ███          ███    █▄  ▀███████████
███   ▄███ ███   ███    ▄█    ███      ███   ▄███   ███    ███ ███    ███   ███          ███          ███    ███   ███    ███
████████▀   ▀█   █▀   ▄████████▀       ████████▀    ███    ███  ▀██████▀   ▄████▀       ▄████▀        ██████████   ███    ███
                                                    ███    ███                                                     ███    ███
"#;
    println!("{title_style}{}{title_style:#}", art);
    start_server(config)
}


fn start_server(config: Cow<ServerConfig>) -> Result<()> {
    let rt = Builder::new_multi_thread()
        .worker_threads(config.server.worker_thread_count as usize)
        .thread_name("WORKER")
        .enable_all()
        .build()?;


    rt.block_on(async {
        let mut complete_block_list: HashSet<Filter> = HashSet::new();

        for source in config.udp_proxy.domain_block_lists.as_ref() {
            trace!("Found block-list source: {}", source);

            match Url::parse(source) {
                Ok(url) => {
                    if (url.scheme().eq("file")) {
                        match fs::read_to_string(source.clone().into_owned()) {
                            Ok(content) => {
                                parse_block_list_content(&mut complete_block_list, content);
                            }
                            Err(err) => {
                                error!("Error occurred while reading file '{}': {}", source, err);
                            }
                        };
                    } else if url.scheme().eq("http") || url.scheme().eq("https") {
                        match reqwest::get(source.clone().into_owned()).await {
                            Ok(res) => {
                                trace!("Got response from block-list source: {}", source);
                                if res.status() == StatusCode::OK {
                                    if let Ok(body) = res.text().await {
                                        parse_block_list_content(&mut complete_block_list, body);
                                    }
                                }
                            }
                            Err(err) => {
                                error!("Error occurred while requesting resource from '{}': {}", source, err);
                            }
                        };
                    }
                }
                Err(_) => {
                    match fs::File::open(source.clone().into_owned()) {
                        Ok(file) => {
                            let mut buf_reader = BufReader::new(file);
                            let mut body = String::new();
                            match buf_reader.read_to_string(&mut body) {
                                Ok(_) => {
                                    parse_block_list_content(&mut complete_block_list, body);
                                }
                                Err(err) => {
                                    error!("Error occurred while reading file '{}': {}", source, err);
                                }
                            }
                        }
                        Err(err) => {
                            error!("Error occurred while reading file '{}': {}", source, err);
                        }
                    }
                }
            };
        }


        let bind = config.udp_proxy.bind.to_string();
        debug!("Block list contains '{}' different domain names.", complete_block_list.len());

        info!("Creating server listening on bind: {}", bind);
        let socket = UdpSocket::bind(bind).await?;
        let arc_socket = Arc::new(socket);

        let proxy = DnsProxy {
            complete_block_list
        };

        let arc_proxy = Arc::new(proxy);

        loop {
            match arc_socket.ready(Interest::READABLE).await {
                Ok(r) => {
                    if r.is_readable() {
                        tokio::spawn(start_reading_from_socket(
                            arc_socket.clone(),
                            arc_proxy.clone(),
                            config.udp_proxy.dns_hosts.clone(),
                            config.udp_proxy.timeout)
                        );
                    }
                }
                Err(err) => {
                    error!("Error trying to read from socket: {}", err);
                }
            }
        }
    })
}

fn parse_block_list_content(complete_block_list: &mut HashSet<Filter>, content: String) {
    let mut filter: Filter;
    for line in content.lines() {
        let split_line: Vec<&str> = line.split_whitespace().collect();

        // we expect lines to follow the pattern of <addr>/s<domain>/n
        if split_line.len() > 1 && split_line.len() < 3 {
            filter = Filter {
                address: split_line.get(0).unwrap().to_string(),
                domain: split_line.get(1).unwrap().to_string(),
            };
            complete_block_list.insert(filter);
        }
    }
}


async fn start_reading_from_socket(socket: Arc<UdpSocket>, proxy: Arc<DnsProxy>, remote_hosts: Cow<'_, [Cow<'_, str>]>, connection_timeout: i64) {
    let mut req = BytePacketBuffer::new();

    let (len, src) = match socket.try_recv_from(&mut req.buf) {
        Ok(r) => r,
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            return;
        }
        Err(err) => {
            error!("Failed to receive message {:?}", err);
            return;
        }
    };

    let res = match proxy.handle(req, len, remote_hosts.as_ref(), connection_timeout).await {
        Ok(data) => data,
        Err(err) => {
            error!("Processing request failed {:?}", err);
            return;
        }
    };

    if let Err(err) = socket.send_to(&res, &src).await {
        error!("Replying to '{}' failed {:?}", &src, err);
    }
}

