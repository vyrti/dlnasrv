use crate::{config::AppConfig, state::AppState};
use crate::platform::network::{NetworkManager, SsdpConfig, PlatformNetworkManager};
use anyhow::Result;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::time::interval;
use tracing::{error, info, warn};

const SSDP_MULTICAST_ADDR: &str = "239.255.255.250";
const SSDP_PORT: u16 = 1900;
const ANNOUNCE_INTERVAL_SECS: u64 = 300; // Announce every 5 minutes

pub fn run_ssdp_service(state: AppState) -> Result<()> {
    let config = state.config;

    // Create platform-specific network manager
    let network_manager = Arc::new(PlatformNetworkManager::new());

    // Task for responding to M-SEARCH requests
    let search_config = config.clone();
    let search_manager = network_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = ssdp_search_responder(search_config, search_manager).await {
            error!("SSDP search responder failed: {}", e);
        }
    });

    // Task for periodically sending NOTIFY announcements
    let announce_config = config.clone();
    let announce_manager = network_manager.clone();
    tokio::spawn(async move {
        ssdp_announcer(announce_config, announce_manager).await;
    });

    info!("SSDP service started with platform abstraction");
    Ok(())
}

async fn ssdp_search_responder(config: Arc<AppConfig>, network_manager: Arc<PlatformNetworkManager>) -> Result<()> {
    const MAX_SOCKET_RETRIES: u32 = 3;
    const MAX_MULTICAST_RETRIES: u32 = 5;
    const RETRY_DELAY_MS: u64 = 1000;

    // Create SSDP socket with retry logic
    let mut socket = None;
    for attempt in 1..=MAX_SOCKET_RETRIES {
        let ssdp_config = SsdpConfig::default();
        match network_manager.create_ssdp_socket_with_config(&ssdp_config).await {
            Ok(s) => {
                info!("Successfully created SSDP socket on port {} (attempt {})", s.port, attempt);
                socket = Some(s);
                break;
            }
            Err(e) => {
                error!("Failed to create SSDP socket (attempt {}): {}", attempt, e);
                if attempt < MAX_SOCKET_RETRIES {
                    warn!("Retrying socket creation in {}ms...", RETRY_DELAY_MS);
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                } else {
                    return Err(anyhow::anyhow!("SSDP socket creation failed after {} attempts: {}", MAX_SOCKET_RETRIES, e));
                }
            }
        }
    }

    let mut socket = socket.unwrap();
    let socket_port = socket.port;

    // Join multicast group with retry logic
    let multicast_addr = SSDP_MULTICAST_ADDR.parse().unwrap();
    let mut multicast_enabled = false;
    
    for attempt in 1..=MAX_MULTICAST_RETRIES {
        match network_manager.join_multicast_group(&mut socket, multicast_addr, None).await {
            Ok(()) => {
                info!("Successfully joined SSDP multicast group on port {} (attempt {})", socket_port, attempt);
                multicast_enabled = true;
                break;
            }
            Err(e) => {
                warn!("Failed to join multicast group (attempt {}): {}", attempt, e);
                if attempt < MAX_MULTICAST_RETRIES {
                    warn!("Retrying multicast join in {}ms...", RETRY_DELAY_MS);
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                } else {
                    error!("Failed to join multicast group after {} attempts, continuing with unicast only", MAX_MULTICAST_RETRIES);
                    break;
                }
            }
        }
    }

    if !multicast_enabled {
        warn!("SSDP search responder running without multicast support - discovery may be limited");
        warn!("Troubleshooting tips:");
        warn!("  - Check firewall settings for UDP port {}", socket_port);
        warn!("  - Ensure network interface supports multicast");
        warn!("  - Try running with elevated privileges if using port < 1024");
    }

    let mut buf = vec![0u8; 2048];
    let mut consecutive_errors = 0;
    const MAX_CONSECUTIVE_ERRORS: u32 = 10;
    
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, addr)) => {
                consecutive_errors = 0; // Reset error counter on success
                let request = String::from_utf8_lossy(&buf[..len]);

                if request.contains("M-SEARCH") && request.contains("ssdp:discover") {
                    info!("Received M-SEARCH from {}", addr);
                    let response = create_ssdp_response(&config, socket_port);
                    
                    // Retry response sending with exponential backoff
                    let mut response_sent = false;
                    for retry in 0..3 {
                        match socket.send_to(response.as_bytes(), addr).await {
                            Ok(_) => {
                                response_sent = true;
                                break;
                            }
                            Err(e) => {
                                warn!("Failed to send M-SEARCH response to {} (attempt {}): {}", addr, retry + 1, e);
                                if retry < 2 {
                                    tokio::time::sleep(Duration::from_millis(100 * (1 << retry))).await;
                                }
                            }
                        }
                    }
                    
                    if !response_sent {
                        error!("Failed to send M-SEARCH response to {} after 3 attempts", addr);
                    }
                }
            }
            Err(e) => {
                consecutive_errors += 1;
                error!("Error receiving SSDP data (consecutive errors: {}): {}", consecutive_errors, e);
                
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    error!("Too many consecutive errors ({}), attempting to recreate socket", MAX_CONSECUTIVE_ERRORS);
                    
                    // Try to recreate the socket
                    let ssdp_config = SsdpConfig::default();
                    match network_manager.create_ssdp_socket_with_config(&ssdp_config).await {
                        Ok(new_socket) => {
                            info!("Successfully recreated SSDP socket on port {}", new_socket.port);
                            socket = new_socket;
                            consecutive_errors = 0;
                            
                            // Try to rejoin multicast group
                            if let Err(e) = network_manager.join_multicast_group(&mut socket, multicast_addr, None).await {
                                warn!("Failed to rejoin multicast group after socket recreation: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to recreate SSDP socket: {}", e);
                            return Err(anyhow::anyhow!("SSDP socket recreation failed: {}", e));
                        }
                    }
                } else {
                    // Exponential backoff for error recovery
                    let delay = std::cmp::min(1000 * (1 << consecutive_errors.min(5)), 30000);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }
}

async fn ssdp_announcer(config: Arc<AppConfig>, network_manager: Arc<PlatformNetworkManager>) {
    let mut interval = interval(Duration::from_secs(ANNOUNCE_INTERVAL_SECS));
    let mut consecutive_failures = 0;
    const MAX_CONSECUTIVE_FAILURES: u32 = 5;
    
    loop {
        interval.tick().await;
        
        match send_ssdp_alive(&config, &network_manager).await {
            Ok(()) => {
                consecutive_failures = 0; // Reset failure counter on success
            }
            Err(e) => {
                consecutive_failures += 1;
                error!("Failed to send SSDP NOTIFY (failure {}): {}", consecutive_failures, e);
                
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    error!("Too many consecutive SSDP announcement failures ({})", MAX_CONSECUTIVE_FAILURES);
                    error!("Troubleshooting suggestions:");
                    error!("  - Check network connectivity");
                    error!("  - Verify firewall allows UDP traffic on SSDP ports");
                    error!("  - Ensure network interfaces are up and support multicast");
                    error!("  - Try restarting the application with elevated privileges");
                    
                    // Reset counter to avoid spam, but continue trying
                    consecutive_failures = 0;
                    
                    // Wait longer before next attempt
                    tokio::time::sleep(Duration::from_secs(30)).await;
                }
            }
        }
    }
}

async fn send_ssdp_alive(config: &AppConfig, network_manager: &PlatformNetworkManager) -> Result<()> {
    const MAX_SOCKET_CREATION_RETRIES: u32 = 3;
    const MAX_SEND_RETRIES: u32 = 3;
    
    info!("Sending SSDP NOTIFY (alive) broadcast");
    
    // Create a temporary socket for announcements with retry logic
    let mut socket = None;
    for attempt in 1..=MAX_SOCKET_CREATION_RETRIES {
        let ssdp_config = SsdpConfig::default();
        match network_manager.create_ssdp_socket_with_config(&ssdp_config).await {
            Ok(s) => {
                socket = Some(s);
                break;
            }
            Err(e) => {
                warn!("Failed to create announcement socket (attempt {}): {}", attempt, e);
                if attempt < MAX_SOCKET_CREATION_RETRIES {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else {
                    error!("Failed to create announcement socket after {} attempts", MAX_SOCKET_CREATION_RETRIES);
                    return Err(anyhow::anyhow!("Announcement socket creation failed: {}", e));
                }
            }
        }
    }

    let socket = socket.unwrap();
    let socket_port = socket.port;

    let message = format!(
        "NOTIFY * HTTP/1.1\r\n\
        HOST: {}:{}\r\n\
        CACHE-CONTROL: max-age=1800\r\n\
        LOCATION: http://{}:{}/description.xml\r\n\
        NT: urn:schemas-upnp-org:device:MediaServer:1\r\n\
        NTS: ssdp:alive\r\n\
        SERVER: OpenDLNA/1.0 UPnP/1.0\r\n\
        USN: uuid:{}::urn:schemas-upnp-org:device:MediaServer:1\r\n\
        SSDP-PORT: {}\r\n\r\n",
        SSDP_MULTICAST_ADDR, SSDP_PORT,
        "127.0.0.1", config.server.port, config.server.uuid, socket_port
    );

    let multicast_addr = format!("{}:{}", SSDP_MULTICAST_ADDR, SSDP_PORT).parse::<SocketAddr>()?;
    
    // Try multicast first with retry logic
    let mut multicast_success = false;
    for attempt in 1..=MAX_SEND_RETRIES {
        match network_manager.send_multicast(&socket, message.as_bytes(), multicast_addr).await {
            Ok(()) => {
                info!("Successfully sent SSDP NOTIFY via multicast (attempt {})", attempt);
                multicast_success = true;
                break;
            }
            Err(e) => {
                warn!("Multicast NOTIFY failed (attempt {}): {}", attempt, e);
                if attempt < MAX_SEND_RETRIES {
                    tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
                }
            }
        }
    }
    
    if !multicast_success {
        warn!("Multicast NOTIFY failed after {} attempts, trying unicast fallback", MAX_SEND_RETRIES);
        
        // Fall back to unicast broadcast on all interfaces with retry logic
        let mut unicast_success = false;
        for attempt in 1..=MAX_SEND_RETRIES {
            match network_manager.send_unicast_fallback(&socket, message.as_bytes(), &socket.interfaces).await {
                Ok(()) => {
                    info!("Successfully sent SSDP NOTIFY via unicast fallback (attempt {})", attempt);
                    unicast_success = true;
                    break;
                }
                Err(e) => {
                    warn!("Unicast fallback failed (attempt {}): {}", attempt, e);
                    if attempt < MAX_SEND_RETRIES {
                        tokio::time::sleep(Duration::from_millis(300 * attempt as u64)).await;
                    }
                }
            }
        }
        
        if !unicast_success {
            error!("Both multicast and unicast fallback failed after {} attempts each", MAX_SEND_RETRIES);
            error!("Platform-specific troubleshooting:");
            
            // Get network diagnostics for better error messages
            if let Ok(diagnostics) = network_manager.get_network_diagnostics().await {
                if !diagnostics.multicast_working {
                    error!("  - Multicast is not working on this system");
                }
                
                if diagnostics.available_ports.is_empty() {
                    error!("  - No ports are available for binding");
                } else {
                    info!("  - Available ports: {:?}", diagnostics.available_ports);
                }
                
                for message in &diagnostics.diagnostic_messages {
                    error!("  - {}", message);
                }
                
                if let Some(firewall) = &diagnostics.firewall_status {
                    if firewall.detected {
                        error!("  - Firewall detected, may be blocking SSDP traffic");
                        for suggestion in &firewall.suggestions {
                            error!("    * {}", suggestion);
                        }
                    }
                }
            }
            
            return Err(anyhow::anyhow!("All SSDP announcement methods failed"));
        }
    }

    Ok(())
}

fn create_ssdp_response(config: &AppConfig, ssdp_port: u16) -> String {
    format!(
        "HTTP/1.1 200 OK\r\n\
        CACHE-CONTROL: max-age=1800\r\n\
        EXT:\r\n\
        LOCATION: http://{}:{}/description.xml\r\n\
        SERVER: OpenDLNA/1.0 UPnP/1.0\r\n\
        ST: urn:schemas-upnp-org:device:MediaServer:1\r\n\
        USN: uuid:{}::urn:schemas-upnp-org:device:MediaServer:1\r\n\
        SSDP-PORT: {}\r\n\r\n",
        "127.0.0.1", config.server.port, config.server.uuid, ssdp_port
    )
}