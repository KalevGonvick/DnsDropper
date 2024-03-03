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

impl Filter {
    fn is_domain_matching(&self, in_domain: &String) -> bool {
        return self.domain.eq(in_domain);
    }
}

pub(crate) fn should_filter(
    domain: &String,
    filter_list: &HashSet<Filter>
) -> bool {
    for filter in filter_list {
        if filter.is_domain_matching(domain) {
            return true;
        }
    }
    return false;
}

pub(crate) async fn load_filtered_domains(
    block_list: &[Cow<'_, str>]
) -> HashSet<Filter> {
    let mut complete_block_list: HashSet<Filter> = HashSet::new();

    for source in block_list {

        if let Ok(url) = Url::parse(source) {

            if url.scheme().eq("file") {

                if let Ok(content) = fs::read_to_string(source.clone().into_owned()) {
                    parse_block_list_content(&mut complete_block_list, content).unwrap();
                }

            } else if url.scheme().eq("http") || url.scheme().eq("https") {

                if let Ok(res) = reqwest::get(source.clone().into_owned()).await {
                    trace!("Got response from block-list source: {}", source);

                    if res.status() == StatusCode::OK {

                        if let Ok(body) = res.text().await {
                            parse_block_list_content(&mut complete_block_list, body).unwrap();
                        }

                    } else {
                        error!("Error! Response was: {}", res.status());
                    }

                } else {
                    error!("Error occurred while requesting resource from '{}'", source);
                };
            }

        } else {
            trace!("Provided string '{}' is not a URL, trying as an external file.", source);

            if let Ok(file) = fs::File::open(source.clone().into_owned()) {
                let mut buf_reader = BufReader::new(file);
                let mut body = String::new();

                if let Ok(_) = buf_reader.read_to_string(&mut body) {
                    parse_block_list_content(&mut complete_block_list, body).unwrap();

                } else if let Err(err) = buf_reader.read_to_string(&mut body) {
                    error!("Error occurred while reading file '{}': {}", source, err);
                }

            } else if let Err(err) = fs::File::open(source.clone().into_owned()) {
                error!("Error occurred while reading file '{}': {}", source, err);
            }
        };
    }

    return complete_block_list;
}

fn parse_block_list_content(
    complete_block_list: &mut HashSet<Filter>,
    content: String
) -> Result<()> {

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