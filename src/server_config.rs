#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(dead_code)]

use std::borrow::Cow;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub struct ServerConfig {
    pub logging: _Config__logging,
    pub server: _Config__server,
    pub udp_proxy: _Config__udp_proxy,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub struct _Config__logging {
    pub enabled: bool,
    pub level: Cow<'static, str>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub struct _Config__server {
    pub worker_thread_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub struct _Config__udp_proxy {
    pub bind: Cow<'static, str>,
    pub dns_hosts: Cow<'static, [Cow<'static, str>]>,
    pub domain_block_lists: Cow<'static, [Cow<'static, str>]>,
    pub packet_size: i64,
    pub record_type_block_list: Cow<'static, [i64]>,
    pub timeout: i64,
}

impl ServerConfig {
    pub fn load() -> Cow<'static, Self> {
        let filepath = concat!(env!("CARGO_MANIFEST_DIR"), "/config/server.yaml");
        Self::load_from(filepath.as_ref()).expect("Failed to load ServerConfig.")
    }

    pub fn load_from(filepath: &::std::path::Path) -> Result<Cow<'static, Self>, Box<dyn ::std::error::Error>> {
        let file_contents = ::std::fs::read_to_string(filepath)?;
        let result: Self = ::serde_yaml::from_str(&file_contents)?;
        Ok(Cow::Owned(result))
    }
}