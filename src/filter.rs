use std::borrow::Cow;
use std::collections::HashSet;
use std::fs;
use std::io::{BufReader, Error, ErrorKind, Read, Result};
use log::{error, trace};
use reqwest::StatusCode;
use url::Url;

#[derive(Hash, Eq, PartialEq, Debug)]
pub(crate) struct Filter {
    pub address: String,
    pub domain: String,
}

pub(crate) fn should_filter(domain: &String, filter_list: &HashSet<Filter>) -> bool {
    for entry in filter_list {
        if &entry.domain == domain {
            return true;
        }
    }
    return false;
}

pub(crate) async fn load_block_list(block_list: &[Cow<'_, str>]) -> HashSet<Filter> {
    let mut complete_block_list: HashSet<Filter> = HashSet::new();

    for source in block_list {
        match Url::parse(source) {
            Ok(url) => {
                if url.scheme().eq("file") {
                    match fs::read_to_string(source.clone().into_owned()) {
                        Ok(content) => {
                            parse_block_list_content(&mut complete_block_list, content)
                                .unwrap_or_else(|error| error!("Error occurred while trying to parse content from provided url resource: {} {}", source, error));
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
                                    parse_block_list_content(&mut complete_block_list, body)
                                        .unwrap_or_else(|error| error!("Error occurred while trying to parse content from provided url resource: {} {}", source, error));
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
                trace!("Provided string '{}' is not a URL, trying as an external file.", source);
                match fs::File::open(source.clone().into_owned()) {
                    Ok(file) => {
                        let mut buf_reader = BufReader::new(file);
                        let mut body = String::new();
                        match buf_reader.read_to_string(&mut body) {
                            Ok(_) => {
                                parse_block_list_content(&mut complete_block_list, body)
                                    .unwrap_or_else(|error| error!("Error occurred while trying to parse content from provided local resource: {} {}", source, error));
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
    return complete_block_list;
}

fn parse_block_list_content(complete_block_list: &mut HashSet<Filter>, content: String) -> Result<()> {
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