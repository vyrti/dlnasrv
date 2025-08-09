use crate::platform::{
    NetworkInterface, InterfaceType, PlatformError, PlatformResult,
    network::{NetworkManager, SsdpSocket, SsdpConfig, NetworkDiagnostics, InterfaceStatus, FirewallStatus}
};
use async_trait::async_trait;
use std::net::{IpAddr, SocketAddr};
use std::process::Command;
use tokio::net::UdpSocket;
use tracing::{debug, info, warn, error};

/// macOS-specific network manager implementation
pub struct MacOSNetworkManager {
    config: SsdpConfig,
}

impl MacOSNetworkManager {
    /// Create a new macOS network manager
    pub fn new() -> Self {
        Self {
            config: SsdpConfig::default(),
        }
    }
    
    /// Create a new macOS network manager with custom configuration
    pub fn with_config(config: SsdpConfig) -> Self {
        Self { config }
    }
    
    /// Check if running with sudo privileges
    fn is_elevated(&self) -> bool {
        std::env::var("USER")
            .map(|user| user == "root")
            .unwrap_or(false) ||
        std::env::var("SUDO_USER").is_ok()
    }
    
    /// Check if a port requires sudo privileges on macOS
    fn requires_elevation(&self, port: u16) -> bool {
        // Ports below 1024 require root privileges on macOS
        port < 1024
    }
    
    /// Try to bind to a port with macOS-specific handling
    async fn try_bind_port_macos(&self, port: u16) -> PlatformResult<UdpSocket> {
        let socket_addr = SocketAddr::from(([0, 0, 0, 0], port));
        
        match UdpSocket::bind(socket_addr).await {
            Ok(socket) => {
                debug!("Successfully bound to port {} on macOS", port);
                Ok(socket)
            }
            Err(e) => {
                if self.requires_elevation(port) && !self.is_elevated() {
                    warn!("Port {} requires sudo privileges on macOS", port);
                    Err(PlatformError::NetworkConfig(format!(
                        "Port {} requires sudo privileges on macOS. Please run with sudo or use a port >= 1024. Error: {}",
                        port, e
                    )))
                } else {
                    Err(PlatformError::NetworkConfig(format!(
                        "Failed to bind to port {} on macOS: {}",
                        port, e
                    )))
                }
            }
        }
    }
    
    /// Detect macOS firewall status
    async fn detect_firewall_status(&self) -> FirewallStatus {
        let mut detected = false;
        let mut blocking_ssdp = None;
        let mut suggestions = Vec::new();
        
        // Check if the application firewall is enabled
        match Command::new("defaults")
            .args(&["read", "/Library/Preferences/com.apple.alf", "globalstate"])
            .output()
        {
            Ok(output) if output.status.success() => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                let state = output_str.trim();
                // 0 = disabled, 1 = enabled for specific services, 2 = enabled for essential services
                detected = state != "0";
                
                if detected {
                    info!("macOS Application Firewall detected (state: {})", state);
                    
                    // For SSDP, we generally assume it might be blocked if firewall is on
                    blocking_ssdp = Some(state == "2"); // More restrictive mode
                    
                    if state != "0" {
                        suggestions.push("Check macOS System Preferences > Security & Privacy > Firewall".to_string());
                        suggestions.push("Add your DLNA application to the firewall exceptions".to_string());
                        suggestions.push("Consider temporarily disabling the firewall to test connectivity".to_string());
                    }
                } else {
                    blocking_ssdp = Some(false);
                }
            }
            _ => {
                warn!("Could not detect macOS firewall status");
                suggestions.push("Unable to detect firewall status. Check System Preferences > Security & Privacy > Firewall".to_string());
            }
        }
        
        if detected {
            suggestions.push("Use 'sudo pfctl -s rules' to check packet filter rules".to_string());
            suggestions.push("Ensure network interfaces allow multicast traffic".to_string());
        }
        
        FirewallStatus {
            detected,
            blocking_ssdp,
            suggestions,
        }
    }
    
    /// Get network interfaces using macOS-specific methods
    async fn get_macos_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();
        
        // Use ifconfig to get interface information
        match Command::new("ifconfig").output() {
            Ok(output) if output.status.success() => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                interfaces = self.parse_ifconfig_output(&output_str)?;
            }
            _ => {
                warn!("Failed to get network interfaces using ifconfig, using fallback");
                // Fallback to basic interface
                interfaces.push(NetworkInterface {
                    name: "en0".to_string(),
                    ip_address: "127.0.0.1".parse().unwrap(),
                    is_loopback: false,
                    is_up: true,
                    supports_multicast: true,
                    interface_type: InterfaceType::Ethernet,
                });
            }
        }
        
        // Filter out loopback interfaces
        interfaces.retain(|iface| !iface.name.starts_with("lo"));
        
        Ok(interfaces)
    }
    
    /// Parse ifconfig output to extract network interface information
    fn parse_ifconfig_output(&self, output: &str) -> PlatformResult<Vec<NetworkInterface>> {
        let mut interfaces = Vec::new();
        let mut current_interface: Option<String> = None;
        let mut current_ip: Option<IpAddr> = None;
        let mut is_up = false;
        let mut supports_multicast = false;
        
        for line in output.lines() {
            let line = line.trim();
            
            // Detect interface name (starts at beginning of line and ends with colon)
            if !line.starts_with('\t') && !line.starts_with(' ') && line.contains(':') {
                // Save previous interface if we have one
                if let (Some(name), Some(ip)) = (&current_interface, &current_ip) {
                    if !name.starts_with("lo") { // Skip loopback
                        let interface_type = self.determine_macos_interface_type(name);
                        interfaces.push(NetworkInterface {
                            name: name.clone(),
                            ip_address: *ip,
                            is_loopback: name.starts_with("lo"),
                            is_up,
                            supports_multicast,
                            interface_type,
                        });
                    }
                }
                
                // Start new interface
                let interface_name = line.split(':').next().unwrap_or("unknown").to_string();
                current_interface = Some(interface_name);
                current_ip = None;
                is_up = false;
                supports_multicast = false;
                
                // Check flags in the same line
                if line.contains("UP") {
                    is_up = true;
                }
                if line.contains("MULTICAST") {
                    supports_multicast = true;
                }
            }
            
            // Look for IP address
            if line.contains("inet ") && !line.contains("inet6") {
                if let Some(ip_part) = line.split_whitespace().nth(1) {
                    if let Ok(ip) = ip_part.parse::<IpAddr>() {
                        current_ip = Some(ip);
                    }
                }
            }
            
            // Check for status flags
            if line.contains("status: active") {
                is_up = true;
            }
        }
        
        // Don't forget the last interface
        if let (Some(name), Some(ip)) = (current_interface, current_ip) {
            if !name.starts_with("lo") { // Skip loopback
                let interface_type = self.determine_macos_interface_type(&name);
                interfaces.push(NetworkInterface {
                    name,
                    ip_address: ip,
                    is_loopback: false,
                    is_up,
                    supports_multicast,
                    interface_type,
                });
            }
        }
        
        Ok(interfaces)
    }
    
    /// Determine interface type based on macOS interface name
    fn determine_macos_interface_type(&self, name: &str) -> InterfaceType {
        if name.starts_with("en") {
            // en0 is typically Ethernet, en1+ can be WiFi or additional Ethernet
            if name == "en0" {
                InterfaceType::Ethernet
            } else {
                // Check if it's WiFi by looking for wireless capabilities
                // This is a simplified heuristic - real implementation might check more thoroughly
                InterfaceType::WiFi
            }
        } else if name.starts_with("utun") || name.starts_with("ipsec") || name.starts_with("ppp") {
            InterfaceType::VPN
        } else if name.starts_with("lo") {
            InterfaceType::Loopback
        } else {
            InterfaceType::Other(name.to_string())
        }
    }
    
    /// Get the preferred network interface for multicast on macOS
    fn get_preferred_multicast_interface<'a>(&self, interfaces: &'a [NetworkInterface]) -> Option<&'a NetworkInterface> {
        // Prioritize en0 (primary Ethernet), then other Ethernet, then WiFi
        interfaces.iter()
            .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
            .min_by_key(|iface| {
                match (&iface.interface_type, iface.name.as_str()) {
                    (InterfaceType::Ethernet, "en0") => 0, // Primary Ethernet
                    (InterfaceType::Ethernet, _) => 1,     // Other Ethernet
                    (InterfaceType::WiFi, _) => 2,         // WiFi
                    (InterfaceType::VPN, _) => 3,          // VPN
                    _ => 4,                                // Other
                }
            })
    }
    
    /// Enable multicast on macOS socket with proper interface selection
    async fn enable_multicast_macos(&self, socket: &mut SsdpSocket, group: IpAddr, interface: Option<&NetworkInterface>) -> PlatformResult<()> {
        let (local_addr, interface_name) = if let Some(iface) = interface {
            (iface.ip_address, iface.name.clone())
        } else {
            // Use the preferred interface for multicast
            let selected_interface = self.get_preferred_multicast_interface(&socket.interfaces)
                .ok_or_else(|| PlatformError::NetworkConfig("No suitable interface for multicast on macOS".to_string()))?;
            (selected_interface.ip_address, selected_interface.name.clone())
        };
        
        match socket.enable_multicast(group, local_addr).await {
            Ok(()) => {
                info!("Successfully enabled multicast on macOS for group {} via interface {} ({})", 
                      group, interface_name, local_addr);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to enable multicast on macOS: {}", e);
                
                // Provide macOS-specific troubleshooting advice
                let mut error_msg = format!("Multicast failed on macOS: {}", e);
                
                if !self.is_elevated() && self.requires_elevation(socket.port) {
                    error_msg.push_str("\nTip: Try running with sudo if using a privileged port.");
                }
                
                error_msg.push_str("\nTip: Check macOS System Preferences > Security & Privacy > Firewall settings.");
                error_msg.push_str("\nTip: Ensure the network interface supports multicast.");
                error_msg.push_str(&format!("\nTip: Try using interface {} explicitly.", interface_name));
                
                Err(PlatformError::NetworkConfig(error_msg))
            }
        }
    }
}

#[async_trait]
impl NetworkManager for MacOSNetworkManager {
    async fn create_ssdp_socket(&self) -> PlatformResult<SsdpSocket> {
        self.create_ssdp_socket_with_config(&self.config).await
    }
    
    async fn create_ssdp_socket_with_config(&self, config: &SsdpConfig) -> PlatformResult<SsdpSocket> {
        let mut last_error = None;
        
        // Try primary port first
        match self.try_bind_port_macos(config.primary_port).await {
            Ok(socket) => {
                let interfaces = self.get_local_interfaces().await?;
                let suitable_interfaces: Vec<_> = interfaces.into_iter()
                    .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
                    .collect();
                
                if suitable_interfaces.is_empty() {
                    return Err(PlatformError::NetworkConfig("No suitable network interfaces found on macOS".to_string()));
                }
                
                return Ok(SsdpSocket {
                    socket,
                    port: config.primary_port,
                    interfaces: suitable_interfaces,
                    multicast_enabled: false,
                });
            }
            Err(e) => {
                warn!("Primary port {} failed on macOS: {}", config.primary_port, e);
                last_error = Some(e);
            }
        }
        
        // Try fallback ports
        for &port in &config.fallback_ports {
            match self.try_bind_port_macos(port).await {
                Ok(socket) => {
                    info!("Using fallback port {} on macOS", port);
                    let interfaces = self.get_local_interfaces().await?;
                    let suitable_interfaces: Vec<_> = interfaces.into_iter()
                        .filter(|iface| !iface.is_loopback && iface.is_up && iface.supports_multicast)
                        .collect();
                    
                    return Ok(SsdpSocket {
                        socket,
                        port,
                        interfaces: suitable_interfaces,
                        multicast_enabled: false,
                    });
                }
                Err(e) => {
                    debug!("Fallback port {} failed on macOS: {}", port, e);
                    last_error = Some(e);
                }
            }
        }
        
        // If we get here, all ports failed
        Err(last_error.unwrap_or_else(|| 
            PlatformError::NetworkConfig("All ports failed on macOS".to_string())
        ))
    }
    
    async fn get_local_interfaces(&self) -> PlatformResult<Vec<NetworkInterface>> {
        self.get_macos_interfaces().await
    }
    
    async fn get_primary_interface(&self) -> PlatformResult<NetworkInterface> {
        let interfaces = self.get_local_interfaces().await?;
        
        // Use the preferred multicast interface
        self.get_preferred_multicast_interface(&interfaces)
            .cloned()
            .ok_or_else(|| PlatformError::NetworkConfig("No suitable primary interface found on macOS".to_string()))
    }
    
    async fn join_multicast_group(&self, socket: &mut SsdpSocket, group: IpAddr, interface: Option<&NetworkInterface>) -> PlatformResult<()> {
        self.enable_multicast_macos(socket, group, interface).await
    }
    
    async fn send_multicast(&self, socket: &SsdpSocket, data: &[u8], group: SocketAddr) -> PlatformResult<()> {
        if !socket.multicast_enabled {
            return Err(PlatformError::NetworkConfig("Multicast not enabled on macOS socket".to_string()));
        }
        
        match socket.send_to(data, group).await {
            Ok(_) => {
                debug!("Sent {} bytes to multicast group {} on macOS", data.len(), group);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send multicast on macOS: {}", e);
                Err(e)
            }
        }
    }
    
    async fn send_unicast_fallback(&self, socket: &SsdpSocket, data: &[u8], interfaces: &[NetworkInterface]) -> PlatformResult<()> {
        let mut success_count = 0;
        let mut last_error = None;
        
        for interface in interfaces {
            // Calculate broadcast address for macOS
            let broadcast_addr = match interface.ip_address {
                IpAddr::V4(ipv4) => {
                    // Simple broadcast calculation - in real implementation, 
                    // you would use route command or system APIs to get proper subnet info
                    let octets = ipv4.octets();
                    let broadcast_ip = std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], 255);
                    SocketAddr::from((broadcast_ip, socket.port))
                }
                IpAddr::V6(_) => {
                    // IPv6 doesn't have broadcast, skip
                    continue;
                }
            };
            
            match socket.send_to(data, broadcast_addr).await {
                Ok(_) => {
                    success_count += 1;
                    debug!("Sent macOS unicast fallback to {} via interface {}", broadcast_addr, interface.name);
                }
                Err(e) => {
                    warn!("Failed to send macOS unicast fallback via interface {}: {}", interface.name, e);
                    last_error = Some(e);
                }
            }
        }
        
        if success_count > 0 {
            info!("macOS unicast fallback succeeded on {} interfaces", success_count);
            Ok(())
        } else {
            Err(last_error.unwrap_or_else(|| 
                PlatformError::NetworkConfig("No macOS interfaces available for unicast fallback".to_string())
            ))
        }
    }
    
    async fn is_port_available(&self, port: u16) -> bool {
        match self.try_bind_port_macos(port).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    
    async fn get_network_diagnostics(&self) -> PlatformResult<NetworkDiagnostics> {
        let interfaces = self.get_local_interfaces().await.unwrap_or_default();
        let mut interface_status = Vec::new();
        let mut available_ports = Vec::new();
        let mut diagnostic_messages = Vec::new();
        
        // Test interfaces
        for interface in interfaces {
            let multicast_capable = self.test_multicast(&interface).await.unwrap_or(false);
            let reachable = interface.is_up && !interface.is_loopback;
            
            let error_message = if !reachable {
                Some("Interface is down or unreachable".to_string())
            } else if !multicast_capable {
                Some("Interface does not support multicast".to_string())
            } else {
                None
            };
            
            interface_status.push(InterfaceStatus {
                interface,
                reachable,
                multicast_capable,
                error_message,
            });
        }
        
        // Test common ports
        for &port in &[1900, 8080, 8081, 8082, 9090] {
            if self.is_port_available(port).await {
                available_ports.push(port);
            } else if port < 1024 && !self.is_elevated() {
                diagnostic_messages.push(format!("Port {} requires sudo privileges on macOS", port));
            }
        }
        
        // Add macOS-specific diagnostic messages
        if available_ports.is_empty() {
            diagnostic_messages.push("No common ports are available for binding on macOS".to_string());
            if !self.is_elevated() {
                diagnostic_messages.push("Consider running with sudo to access privileged ports".to_string());
            }
        }
        
        if interface_status.iter().all(|status| !status.multicast_capable) {
            diagnostic_messages.push("No macOS interfaces support multicast".to_string());
            diagnostic_messages.push("Check network interface configuration and drivers".to_string());
        }
        
        // Check for preferred interface
        let interfaces_vec: Vec<NetworkInterface> = interface_status.iter().map(|s| s.interface.clone()).collect();
        if let Some(preferred) = self.get_preferred_multicast_interface(&interfaces_vec) {
            diagnostic_messages.push(format!("Preferred multicast interface: {} ({})", preferred.name, preferred.ip_address));
        }
        
        // Get firewall status
        let firewall_status = Some(self.detect_firewall_status().await);
        
        Ok(NetworkDiagnostics {
            multicast_working: interface_status.iter().any(|status| status.multicast_capable),
            available_ports,
            interface_status,
            diagnostic_messages,
            firewall_status,
        })
    }
    
    async fn test_multicast(&self, interface: &NetworkInterface) -> PlatformResult<bool> {
        // Basic test for macOS - check if interface supports multicast
        if !interface.supports_multicast || !interface.is_up || interface.is_loopback {
            return Ok(false);
        }
        
        // Try to create a test socket and join multicast group
        match UdpSocket::bind("0.0.0.0:0").await {
            Ok(test_socket) => {
                match interface.ip_address {
                    IpAddr::V4(local_v4) => {
                        let multicast_addr = "239.255.255.250".parse::<std::net::Ipv4Addr>().unwrap();
                        match test_socket.join_multicast_v4(multicast_addr, local_v4) {
                            Ok(()) => {
                                debug!("Multicast test successful on macOS interface {}", interface.name);
                                Ok(true)
                            }
                            Err(e) => {
                                debug!("Multicast test failed on macOS interface {}: {}", interface.name, e);
                                Ok(false)
                            }
                        }
                    }
                    IpAddr::V6(_) => {
                        // IPv6 multicast test would go here
                        Ok(true) // Assume it works for now
                    }
                }
            }
            Err(_) => Ok(false),
        }
    }
}

impl Default for MacOSNetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_macos_network_manager_creation() {
        let manager = MacOSNetworkManager::new();
        assert_eq!(manager.config.primary_port, 1900);
    }
    
    #[test]
    fn test_requires_elevation() {
        let manager = MacOSNetworkManager::new();
        assert!(manager.requires_elevation(80));
        assert!(manager.requires_elevation(443));
        assert!(!manager.requires_elevation(8080));
        assert!(!manager.requires_elevation(9090));
    }
    
    #[test]
    fn test_interface_type_determination() {
        let manager = MacOSNetworkManager::new();
        
        assert_eq!(
            manager.determine_macos_interface_type("en0"),
            InterfaceType::Ethernet
        );
        
        assert_eq!(
            manager.determine_macos_interface_type("en1"),
            InterfaceType::WiFi
        );
        
        assert_eq!(
            manager.determine_macos_interface_type("utun0"),
            InterfaceType::VPN
        );
        
        assert_eq!(
            manager.determine_macos_interface_type("lo0"),
            InterfaceType::Loopback
        );
    }
    
    #[tokio::test]
    async fn test_port_availability_check() {
        let manager = MacOSNetworkManager::new();
        
        // Test with a high port that should be available
        let available = manager.is_port_available(8080).await;
        // This might fail in test environment, but we can at least verify the method works
        println!("Port 8080 available: {}", available);
    }
    
    #[test]
    fn test_ifconfig_parsing() {
        let manager = MacOSNetworkManager::new();
        
        let sample_output = r#"
lo0: flags=8049<UP,LOOPBACK,RUNNING,MULTICAST> mtu 16384
	inet 127.0.0.1 netmask 0xff000000
en0: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500
	inet 192.168.1.100 netmask 0xffffff00 broadcast 192.168.1.255
	status: active
en1: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500
	inet 192.168.1.101 netmask 0xffffff00 broadcast 192.168.1.255
	status: active
"#;
        
        let interfaces = manager.parse_ifconfig_output(sample_output).unwrap();
        assert_eq!(interfaces.len(), 2); // lo0 should be filtered out
        
        let en0 = &interfaces[0];
        assert_eq!(en0.name, "en0");
        assert_eq!(en0.ip_address, "192.168.1.100".parse::<IpAddr>().unwrap());
        assert_eq!(en0.interface_type, InterfaceType::Ethernet);
        assert!(en0.is_up);
        assert!(en0.supports_multicast);
        
        let en1 = &interfaces[1];
        assert_eq!(en1.name, "en1");
        assert_eq!(en1.ip_address, "192.168.1.101".parse::<IpAddr>().unwrap());
        assert_eq!(en1.interface_type, InterfaceType::WiFi);
    }
    
    #[test]
    fn test_preferred_interface_selection() {
        let manager = MacOSNetworkManager::new();
        
        let interfaces = vec![
            NetworkInterface {
                name: "en1".to_string(),
                ip_address: "192.168.1.101".parse().unwrap(),
                is_loopback: false,
                is_up: true,
                supports_multicast: true,
                interface_type: InterfaceType::WiFi,
            },
            NetworkInterface {
                name: "en0".to_string(),
                ip_address: "192.168.1.100".parse().unwrap(),
                is_loopback: false,
                is_up: true,
                supports_multicast: true,
                interface_type: InterfaceType::Ethernet,
            },
            NetworkInterface {
                name: "utun0".to_string(),
                ip_address: "10.0.0.1".parse().unwrap(),
                is_loopback: false,
                is_up: true,
                supports_multicast: true,
                interface_type: InterfaceType::VPN,
            },
        ];
        
        let preferred = manager.get_preferred_multicast_interface(&interfaces);
        assert!(preferred.is_some());
        assert_eq!(preferred.unwrap().name, "en0"); // Should prefer en0 (primary Ethernet)
    }
}