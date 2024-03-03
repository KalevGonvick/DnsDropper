#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(dead_code)]

use std::borrow::Cow;

#[derive(Debug, Clone)]
#[allow(non_camel_case_types)]
pub struct InternalConfig {
    pub default_server_config_dir: Cow<'static, str>,
    pub max_udp_packet_size: i64,
    pub worker_thread_name: Cow<'static, str>,
}

pub const INTERNAL_CONFIG: InternalConfig = InternalConfig {
    default_server_config_dir: Cow::Borrowed("config/server.yaml"),
    max_udp_packet_size: 4096,
    worker_thread_name: Cow::Borrowed("WT"),
};
