use crate::{config::Config, state::AppState};
use anyhow::Result;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::UdpSocket, time::interval};
use tracing::{error, info, warn};

const SSDP_ADDR: &str = "239.255.255.250:1900";
const SSDP_PORT: u16 = 1900;
const ANNOUNCE_INTERVAL_SECS: u64 = 300; // Announce every 5 minutes

pub fn run_ssdp_service(state: AppState) -> Result<()> {
    let config = state.config;

    // Task for responding to M-SEARCH requests
    let search_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = ssdp_search_responder(search_config).await {
            error!("SSDP search responder failed: {}", e);
        }
    });

    // Task for periodically sending NOTIFY announcements
    let announce_config = config.clone();
    tokio::spawn(async move {
        ssdp_announcer(announce_config).await;
    });

    info!("SSDP service started");
    Ok(())
}

async fn ssdp_search_responder(config: Arc<Config>) -> Result<()> {
    let socket = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], SSDP_PORT))).await?;
    let multicast_addr = "239.255.255.250".parse()?;
    let local_addr = "0.0.0.0".parse()?;
    socket.join_multicast_v4(multicast_addr, local_addr)?;

    let mut buf = vec![0u8; 2048];
    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let request = String::from_utf8_lossy(&buf[..len]);

        if request.contains("M-SEARCH") && request.contains("ssdp:discover") {
            info!("Received M-SEARCH from {}", addr);
            let response = create_ssdp_response(&config);
            socket.send_to(response.as_bytes(), addr).await?;
        }
    }
}

async fn ssdp_announcer(config: Arc<Config>) {
    let mut interval = interval(Duration::from_secs(ANNOUNCE_INTERVAL_SECS));
    loop {
        interval.tick().await;
        if let Err(e) = send_ssdp_alive(&config).await {
            warn!("Failed to send SSDP NOTIFY: {}", e);
        }
    }
}

async fn send_ssdp_alive(config: &Config) -> Result<()> {
    info!("Sending SSDP NOTIFY (alive) broadcast");
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let message = format!(
        "NOTIFY * HTTP/1.1\r\n\
        HOST: {SSDP_ADDR}\r\n\
        CACHE-CONTROL: max-age=1800\r\n\
        LOCATION: http://{}:{}/description.xml\r\n\
        NT: urn:schemas-upnp-org:device:MediaServer:1\r\n\
        NTS: ssdp:alive\r\n\
        SERVER: Rust DLNA/1.0 UPnP/1.0\r\n\
        USN: uuid:{}::urn:schemas-upnp-org:device:MediaServer:1\r\n\r\n",
        config.host, config.port, config.uuid
    );

    socket.send_to(message.as_bytes(), SSDP_ADDR).await?;
    Ok(())
}

fn create_ssdp_response(config: &Config) -> String {
    format!(
        "HTTP/1.1 200 OK\r\n\
        CACHE-CONTROL: max-age=1800\r\n\
        EXT:\r\n\
        LOCATION: http://{}:{}/description.xml\r\n\
        SERVER: Rust DLNA/1.0 UPnP/1.0\r\n\
        ST: urn:schemas-upnp-org:device:MediaServer:1\r\n\
        USN: uuid:{}::urn:schemas-upnp-org:device:MediaServer:1\r\n\r\n",
        config.host, config.port, config.uuid
    )
}