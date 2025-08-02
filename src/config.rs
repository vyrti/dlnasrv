use anyhow::{Context, Result};
use clap::Parser;
use std::{
    net::IpAddr,
    path::PathBuf,
};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The directory containing media files to serve
    #[arg(default_value_t = std::env::current_dir().unwrap().to_string_lossy().to_string())]
    media_dir: String,

    /// The network port to listen on
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// The friendly name for the DLNA server
    #[arg(short, long, default_value = "DLNA")]
    name: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub name: String,
    pub uuid: String,
    pub media_dir: PathBuf,
    pub host: IpAddr,
    pub port: u16,
}

impl Config {
    pub async fn from_args() -> Result<Self> {
        let args = Args::parse();
        let host = get_local_ip().context("Could not determine local IP address")?;
        let media_dir = PathBuf::from(args.media_dir);

        if !media_dir.is_dir() {
            anyhow::bail!("Media path is not a valid directory: {}", media_dir.display());
        }

        Ok(Config {
            name: args.name,
            uuid: Uuid::new_v4().to_string(),
            media_dir,
            host,
            port: args.port,
        })
    }
}

/// Finds the local IP address of the machine.
fn get_local_ip() -> Option<IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}