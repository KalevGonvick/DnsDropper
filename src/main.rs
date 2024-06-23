use std::io::{Error, ErrorKind};
use tokio::io::Result;
use clap::Parser;
use server::config::internal::INTERNAL_CONFIG;
use server::config::server_config::ServerConfig;
use server::Server;


fn main() -> Result<()> {
    let args = ClArgs::parse();
    let mut config_dir: String = INTERNAL_CONFIG.default_server_config_dir.to_string();
    let config_args = args.config;

    if !config_args.is_empty() {
        config_dir = config_args;
    }

    match ServerConfig::load_from(std::path::Path::new(&config_dir)) {
        Ok(server_config) => {
            logger::setup(server_config.logging.level.as_ref());
            logger::print_title();
            let server: Server = Server::new(server_config);
            server.start()
        }

        Err(_) => {
            Err(Error::new(ErrorKind::InvalidInput, std::format!("Failed to read server.yaml from the provided path: {}", config_dir)))
        }
    }
}



// A customizable light-weight DNS proxy with domain filtering capabilities.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct ClArgs {

    // Path to the template file.
    #[arg(short, long)]
    config: String
}
