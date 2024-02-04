use std::collections::HashSet;
use log::info;
use crate::logging;
use crate::logging::HighlightStyle::DefaultHighlight;

#[derive(Hash, Eq, PartialEq, Debug)]
pub(crate) struct Filter {
    pub address: String,
    pub domain: String
}

impl Filter {
    fn matches(&self, address: String, domain: String) -> bool {
        return address.eq(&self.address) && domain.eq(&self.domain);
    }
}

pub(crate) fn should_filter(domain: String, filter_list: &HashSet<Filter>) -> bool {
    let style = logging::get_highlight_style(DefaultHighlight);
    for entry in filter_list {
        if entry.domain == domain {
            info!("Block-List contains the name '{style}{}{style:#}'", domain);
            return true;
        }
    }
    return false;
}

#[cfg(test)]
mod tests {
    use crate::dns_proxy::DnsProxy;
    use super::*;

    #[test]
    fn filter_test() {
        let proxy: DnsProxy = DnsProxy {
            complete_block_list: HashSet::new(),
        };
    }
}