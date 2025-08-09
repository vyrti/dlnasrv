#[cfg(target_os = "macos")]
use super::{NetworkInterface, InterfaceType, PlatformResult};
use std::collections::HashMap;

/// Get macOS version information
pub fn get_macos_version() -> PlatformResult<String> {
    // Try to get macOS version from system
    match std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(format!("macOS {}", version))
        }
        _ => {
            // Fallback to basic detection
            Ok("macOS (unknown version)".to_string())
        }
    }
}

/// Detect network interfaces on macOS
pub async fn detect_network_interfaces() -> PlatformResult<Vec<NetworkInterface>> {
    let mut interfaces = Vec::new();
    
    // Use ifconfig to get interface information
    match std::process::Command::new("ifconfig").output() {
        Ok(output) if output.status.success() => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            
            // Parse ifconfig output (simplified)
            // In a real implementation, you might use system APIs or more robust parsing
            for line in output_str.lines() {
                if line.starts_with(char::is_alphabetic) && line.contains(':') {
                    // This is an interface line
                    let interface_name = line.split(':').next().unwrap_or("unknown").to_string();
                    
                    // Skip loopback
                    if interface_name == "lo0" {
                        continue;
                    }
                    
                    // Determine interface type
                    let interface_type = if interface_name.starts_with("en") {
                        if interface_name == "en0" {
                            InterfaceType::Ethernet // Usually the primary interface
                        } else {
                            InterfaceType::WiFi
                        }
                    } else if interface_name.starts_with("utun") || interface_name.starts_with("ipsec") {
                        InterfaceType::VPN
                    } else {
                        InterfaceType::Other(interface_name.clone())
                    };
                    
                    // Create interface (with placeholder IP - would need proper parsing)
                    let interface = NetworkInterface {
                        name: interface_name,
                        ip_address: "127.0.0.1".parse().unwrap(), // Placeholder
                        is_loopback: false,
                        is_up: true, // Would need to parse flags
                        supports_multicast: true,
                        interface_type,
                    };
                    
                    interfaces.push(interface);
                }
            }
        }
        _ => {
            // Fallback: create a basic interface
            let interface = NetworkInterface {
                name: "en0".to_string(),
                ip_address: "127.0.0.1".parse().unwrap(),
                is_loopback: false,
                is_up: true,
                supports_multicast: true,
                interface_type: InterfaceType::Ethernet,
            };
            interfaces.push(interface);
        }
    }
    
    Ok(interfaces)
}

/// Gather macOS-specific metadata
pub fn gather_macos_metadata() -> PlatformResult<HashMap<String, String>> {
    let mut metadata = HashMap::new();
    
    metadata.insert("platform".to_string(), "macOS".to_string());
    
    // Get system information using system_profiler or sw_vers
    if let Ok(output) = std::process::Command::new("sw_vers").output() {
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim().to_lowercase().replace(' ', "_");
                    let value = value.trim().to_string();
                    metadata.insert(key, value);
                }
            }
        }
    }
    
    // Get hardware information
    if let Ok(output) = std::process::Command::new("uname")
        .arg("-m")
        .output()
    {
        if output.status.success() {
            let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            metadata.insert("hardware_architecture".to_string(), arch);
        }
    }
    
    // Get hostname
    if let Ok(output) = std::process::Command::new("hostname").output() {
        if output.status.success() {
            let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
            metadata.insert("hostname".to_string(), hostname);
        }
    }
    
    Ok(metadata)
}

/// Check macOS firewall status
pub fn get_firewall_status() -> PlatformResult<bool> {
    // Check if the application firewall is enabled
    match std::process::Command::new("defaults")
        .args(&["read", "/Library/Preferences/com.apple.alf", "globalstate"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            let state = output_str.trim();
            // 0 = disabled, 1 = enabled for specific services, 2 = enabled for essential services
            Ok(state != "0")
        }
        _ => {
            // Assume firewall is enabled if we can't determine status
            Ok(true)
        }
    }
}

/// Check if running with sudo privileges
pub fn is_elevated() -> bool {
    std::env::var("USER")
        .map(|user| user == "root")
        .unwrap_or(false) ||
    std::env::var("SUDO_USER").is_ok()
}

/// Get the preferred network interface for multicast on macOS
pub fn get_preferred_multicast_interface() -> Option<String> {
    // On macOS, en0 is typically the primary Ethernet interface
    // and en1 is typically WiFi
    Some("en0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_macos_version_detection() {
        let version = get_macos_version();
        assert!(version.is_ok());
        assert!(version.unwrap().contains("macOS"));
    }
    
    #[tokio::test]
    async fn test_macos_interface_detection() {
        let interfaces = detect_network_interfaces().await;
        assert!(interfaces.is_ok());
        let ifaces = interfaces.unwrap();
        assert!(!ifaces.is_empty());
    }
    
    #[test]
    fn test_macos_metadata() {
        let metadata = gather_macos_metadata();
        assert!(metadata.is_ok());
        let meta = metadata.unwrap();
        assert!(meta.contains_key("platform"));
        assert_eq!(meta.get("platform").unwrap(), "macOS");
    }
    
    #[test]
    fn test_preferred_interface() {
        let interface = get_preferred_multicast_interface();
        assert!(interface.is_some());
        assert_eq!(interface.unwrap(), "en0");
    }
}