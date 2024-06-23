use std::borrow::Cow;
use std::collections::HashSet;
use std::{fs, io};
use std::future::Future;
use std::io::{BufReader, Error, ErrorKind, Read};
use std::pin::Pin;
use std::sync::Arc;
use log::{error, trace};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use url::Url;
use packet::BytePacketBuffer;
use crate::config::server_config::ServerConfig;
use crate::exchange::Exchange;
use crate::packet_handler::PacketHandler;

struct FilterHandler {
    block_list_sources: Vec<String>
}

impl PacketHandler for FilterHandler {
    fn exec(&self, exchange: &mut Exchange) {
        todo!()
    }
}

impl FilterHandler {
    pub fn new(block_list: &[Cow<'_, str>]) -> FilterHandler {
        let mut real_list: Vec<String>  = Vec::new();
        for borrowed_str in block_list {
            real_list.push(match borrowed_str {
                Cow::Borrowed(s) => {
                    s.to_owned().to_string()
                }
                Cow::Owned(s) => {
                    s.to_string()
                }
            });
        }
        FilterHandler {
            block_list_sources: real_list
        }
    }

    pub async fn resolved_block_list(&self) -> HashSet<Filter> {
        let mut complete_block_list: HashSet<Filter> = HashSet::new();
        for source in &self.block_list_sources {
            if let Ok(url) = Url::parse(source.as_str()) {
                if url.scheme().eq("file") {
                    if let Ok(content) = fs::read_to_string(source) {
                        self.parse_block_list_content(&mut complete_block_list, content).unwrap();
                    }
                } else if url.scheme().eq("http") || url.scheme().eq("https") {
                    self.handle_url_scheme(source.clone(), &mut complete_block_list).await;
                }

            } else {
                trace!("Provided string '{}' is not a URL, trying as an external file.", source);
                self.handle_file_scheme(source.clone(), &mut complete_block_list).await;
            };
        }

        return complete_block_list;
    }

    async fn handle_file_scheme(&self, source: String, complete_block_list: &mut HashSet<Filter>) {
        match fs::File::open(source.clone()) {
            Ok(file) => {
                let mut buf_reader = BufReader::new(file);
                let mut body = String::new();
                match buf_reader.read_to_string(&mut body) {
                    Ok(_) => {
                        self.parse_block_list_content(complete_block_list, body).unwrap();
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

    async fn handle_url_scheme(&self, source: String, complete_block_list: &mut HashSet<Filter>) {
        match reqwest::get(source.clone()).await {
            Ok(res) => {
                trace!("Got response from block-list source: {}", source);
                if res.status() == StatusCode::OK {
                    if let Ok(body) = res.text().await {
                        self.parse_block_list_content(complete_block_list, body).unwrap();
                    }
                } else {
                    error!("Error! Response was: {}", res.status());
                }
            }
            Err(..) => {
                error!("Error occurred while requesting resource from '{}'", source);
            }
        };
    }

    fn parse_block_list_content(&self, complete_block_list: &mut HashSet<Filter>, content: String) -> std::io::Result<()> {
        let mut filter: Filter;
        for line in content.lines() {
            let split_line: Vec<&str> = line.split_whitespace().collect();

            // we expect lines to follow the pattern of <addr>/s<domain>/n
            // TODO make it more relaxed format wise. regex?
            if split_line.len() > 1 && split_line.len() < 3 {
                filter = Filter {
                    address: match split_line.get(0) {
                        Some(x) => x,
                        None => return Err(Error::new(ErrorKind::InvalidInput, "")),
                    }.to_string(),

                    domain: match split_line.get(1) {
                        Some(x) => x,
                        None => return Err(Error::new(ErrorKind::InvalidInput, "")),
                    }.to_string(),
                };
                complete_block_list.insert(filter);
            }
        }

        Ok(())
    }
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct Filter {
    pub address: String,
    pub domain: String,
}

impl Filter {
    fn is_domain_matching(&self, in_domain: &String) -> bool {
        return self.domain.eq(in_domain);
    }
}

pub fn should_filter(domain: &String, filter_list: &HashSet<Filter>) -> bool {
    for filter in filter_list {
        if filter.is_domain_matching(domain) {
            return true;
        }
    }
    return false;
}