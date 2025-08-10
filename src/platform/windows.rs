#[cfg(target_os = "windows")]
use super::{InterfaceType, NetworkInterface, PlatformError, PlatformResult};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::IpAddr;
use std::process::Command;
use tracing::{debug, info, warn};

/// Get Windows version information
pub fn get_windows_version() -> PlatformResult<String> {
    // Use std::env to get basic version info
    // In a real implementation, you might use Windows APIs for more detailed info
    match std::env::var("OS") {
        Ok(os) if os.contains("Windows") => {
            // Try to get more specific version info from environment
            let version = std::env::var("PROCESSOR_ARCHITECTURE")
                .map(|arch| format!("Windows ({})", arch))
                .unwrap_or_else(|_| "Windows".to_string());
            Ok(version)
        }
        _ => Ok("Windows (unknown version)".to_string()),
    }
}

/// Data structure to deserialize the JSON output from our PowerShell command.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct PsNetAdapterInfo {
    name: String,
    description: String,
    #[serde(rename = "IPAddress")]
    ip_address: String,
    #[serde(rename = "InterfaceType")]
    if_type: Option<u32>,
}

/// Maps IANA interface types (from PowerShell) to our internal InterfaceType enum.
fn map_iana_if_type(if_type: Option<u32>) -> InterfaceType {
    match if_type {
        Some(6) => InterfaceType::Ethernet,
        Some(71) => InterfaceType::WiFi,
        Some(1) | Some(24) => InterfaceType::Loopback, // 1 is 'other', 24 is softwareLoopback
        Some(131) | Some(237) => InterfaceType::VPN,   // 131 is tunnel, 237 is pptp
        Some(val) => InterfaceType::Other(format!("ifType {}", val)),
        None => InterfaceType::Other("ifType null".to_string()),
    }
}

/// Detect network interfaces on Windows by executing a PowerShell command.
pub async fn detect_network_interfaces() -> PlatformResult<Vec<NetworkInterface>> {
    // This PowerShell command gets active, configured IPv4 network adapters and outputs them as JSON.
    // - Filters for IPv4 addresses from DHCP or Manual configuration (ignores loopback/APIPA).
    // - Joins with NetAdapter info to get Name, Description, and Type.
    // - Filters for adapters that are actually 'Up'.
    let command = r#"
        Get-NetIPAddress -AddressFamily IPv4 | Where-Object {
            $_.PrefixOrigin -in @('Manual', 'Dhcp') -and $_.InterfaceAlias -notlike 'Loopback*'
        } | ForEach-Object {
            $ip = $_
            Get-NetAdapter -InterfaceIndex $ip.InterfaceIndex | Where-Object {
                $_.Status -eq 'Up'
            } | ForEach-Object {
                [PSCustomObject]@{
                    Name = $_.Name
                    Description = $_.InterfaceDescription
                    IPAddress = $ip.IPAddress
                    InterfaceType = $_.ifType
                }
            }
        } | ConvertTo-Json
    "#;

    let output = match Command::new("powershell")
        .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", command])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            warn!("Failed to execute PowerShell for network detection: {}. Falling back to ipconfig.", e);
            return detect_interfaces_fallback().await;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("PowerShell command for network detection failed: {}. Falling back to ipconfig.", stderr);
        return detect_interfaces_fallback().await;
    }

    let json_output = String::from_utf8_lossy(&output.stdout);

    // The output might be a single object or an array of objects.
    let adapters: Vec<PsNetAdapterInfo> = if json_output.trim().starts_with('[') {
        serde_json::from_str(&json_output).unwrap_or_else(|e| {
            warn!("Failed to parse PowerShell JSON array: {}", e);
            Vec::new()
        })
    } else if json_output.trim().starts_with('{') {
        serde_json::from_str(&json_output)
            .map(|obj| vec![obj])
            .unwrap_or_else(|e| {
                warn!("Failed to parse PowerShell JSON object: {}", e);
                Vec::new()
            })
    } else {
        if !json_output.trim().is_empty() {
            warn!("PowerShell output was not valid JSON: {}", json_output);
        }
        Vec::new()
    };

    if adapters.is_empty() {
        warn!("PowerShell command returned no network adapters. Falling back to ipconfig.");
        return detect_interfaces_fallback().await;
    }

    let interfaces: Vec<NetworkInterface> = adapters
        .into_iter()
        .filter_map(|adapter| {
            match adapter.ip_address.parse::<IpAddr>() {
                Ok(ip) => {
                    let interface_type = map_iana_if_type(adapter.if_type);
                    if interface_type == InterfaceType::Loopback {
                        return None;
                    }
                    Some(NetworkInterface {
                        name: adapter.name,
                        ip_address: ip,
                        is_loopback: false,
                        is_up: true,
                        supports_multicast: true, // Assume true for active adapters
                        interface_type,
                    })
                }
                Err(_) => None,
            }
        })
        .collect();

    if interfaces.is_empty() {
        warn!("No valid interfaces found after parsing PowerShell output. Falling back to ipconfig.");
        return detect_interfaces_fallback().await;
    }

    info!(
        "Successfully detected {} network interface(s) via PowerShell.",
        interfaces.len()
    );
    Ok(interfaces)
}

/// Fallback method to detect network interfaces using `ipconfig /all`.
async fn detect_interfaces_fallback() -> PlatformResult<Vec<NetworkInterface>> {
    let output = match Command::new("ipconfig").arg("/all").output() {
        Ok(output) if output.status.success() => output,
        Err(e) => {
            return Err(PlatformError::DetectionFailed(format!(
                "Fallback 'ipconfig /all' failed to execute: {}",
                e
            )));
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PlatformError::DetectionFailed(format!(
                "Fallback 'ipconfig /all' command failed with status {}: {}",
                output.status, stderr
            )));
        }
    };
    let output_str = String::from_utf8_lossy(&output.stdout);
    parse_ipconfig_output(&output_str)
}

fn parse_ipconfig_output(output: &str) -> PlatformResult<Vec<NetworkInterface>> {
    let mut interfaces = Vec::new();
    let mut current_interface: Option<PartialInterface> = None;

    for line in output.lines() {
        if line.trim().is_empty() {
            if let Some(iface) = current_interface.take() {
                if let Some(complete_iface) = iface.build() {
                    interfaces.push(complete_iface);
                }
            }
            continue;
        }

        if !line.starts_with(' ') && line.contains(':') {
            if let Some(iface) = current_interface.take() {
                if let Some(complete_iface) = iface.build() {
                    interfaces.push(complete_iface);
                }
            }
            let name = line.trim_end_matches(':').to_string();
            debug!("(ipconfig) Found potential adapter section: '{}'", name);
            current_interface = Some(PartialInterface::new(name));
            continue;
        }

        if let Some(iface) = current_interface.as_mut() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    s if s.contains("IPv4 Address") => {
                        if let Ok(ip) = value.split('(').next().unwrap_or(value).trim().parse() {
                            iface.ip_address = Some(ip);
                        }
                    }
                    "Description" => {
                        iface.description = Some(value.to_string());
                    }
                    "Media State" => {
                        if value.contains("disconnected") {
                            iface.is_up = Some(false);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(iface) = current_interface.take() {
        if let Some(complete_iface) = iface.build() {
            interfaces.push(complete_iface);
        }
    }

    Ok(interfaces)
}

#[derive(Default)]
struct PartialInterface {
    name: String,
    description: Option<String>,
    ip_address: Option<IpAddr>,
    is_up: Option<bool>,
}

impl PartialInterface {
    fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    fn build(self) -> Option<NetworkInterface> {
        let ip_address = match self.ip_address {
            Some(ip) => ip,
            None => {
                debug!(
                    "(ipconfig) Discarding adapter '{}': No IPv4 address found.",
                    self.name
                );
                return None;
            }
        };

        let is_up = self.is_up.unwrap_or(true);
        if !is_up {
            debug!(
                "(ipconfig) Discarding adapter '{}': Media is disconnected.",
                self.name
            );
            return None;
        }

        let name_lower = self.name.to_lowercase();
        let desc_lower = self.description.as_deref().unwrap_or("").to_lowercase();

        let is_loopback = name_lower.contains("loopback") || desc_lower.contains("loopback");
        if is_loopback {
            debug!(
                "(ipconfig) Discarding adapter '{}': Is a loopback interface.",
                self.name
            );
            return None;
        }

        let interface_type =
            determine_windows_interface_type(self.description.as_deref().unwrap_or(&self.name));

        Some(NetworkInterface {
            name: self.name,
            ip_address,
            is_loopback,
            is_up,
            supports_multicast: !is_loopback,
            interface_type,
        })
    }
}

fn determine_windows_interface_type(description: &str) -> InterfaceType {
    let desc_lower = description.to_lowercase();
    if desc_lower.contains("ethernet")
        || desc_lower.contains("gigabit")
        || desc_lower.contains("local area connection")
    {
        InterfaceType::Ethernet
    } else if desc_lower.contains("wi-fi")
        || desc_lower.contains("wireless")
        || desc_lower.contains("802.11")
    {
        InterfaceType::WiFi
    } else if desc_lower.contains("vpn")
        || desc_lower.contains("tap")
        || desc_lower.contains("tun")
        || desc_lower.contains("wan miniport")
    {
        InterfaceType::VPN
    } else if desc_lower.contains("loopback") {
        InterfaceType::Loopback
    } else {
        InterfaceType::Other(description.to_string())
    }
}

/// Gather Windows-specific metadata
pub fn gather_windows_metadata() -> PlatformResult<HashMap<String, String>> {
    let mut metadata = HashMap::new();

    // Add Windows-specific environment variables
    if let Ok(computer_name) = std::env::var("COMPUTERNAME") {
        metadata.insert("computer_name".to_string(), computer_name);
    }

    if let Ok(user_domain) = std::env::var("USERDOMAIN") {
        metadata.insert("user_domain".to_string(), user_domain);
    }

    if let Ok(processor_arch) = std::env::var("PROCESSOR_ARCHITECTURE") {
        metadata.insert("processor_architecture".to_string(), processor_arch);
    }

    if let Ok(number_of_processors) = std::env::var("NUMBER_OF_PROCESSORS") {
        metadata.insert("number_of_processors".to_string(), number_of_processors);
    }

    // Add Windows version detection
    metadata.insert("platform".to_string(), "Windows".to_string());

    Ok(metadata)
}

/// Check if running with administrator privileges
pub fn is_elevated() -> bool {
    // This is a simplified check
    // In a real implementation, you would use Windows APIs to check for admin privileges
    std::env::var("USERNAME")
        .map(|username| username.to_lowercase().contains("admin"))
        .unwrap_or(false)
}

/// Get Windows firewall status
pub fn get_firewall_status() -> PlatformResult<bool> {
    // This would use Windows APIs to check firewall status
    // For now, assume firewall is active on Windows
    Ok(true)
}

/// Check if a port requires elevation on Windows
pub fn requires_elevation(port: u16) -> bool {
    // Ports below 1024 typically require administrator privileges on Windows
    port < 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_version_detection() {
        let version = get_windows_version();
        assert!(version.is_ok());
        assert!(version.unwrap().contains("Windows"));
    }

    #[tokio::test]
    async fn test_windows_interface_detection() {
        let interfaces = detect_network_interfaces().await;
        // In a test environment, ipconfig might not be available or might fail.
        // We are checking that it returns a result, which might be an error.
        // On a typical Windows machine, this should be Ok.
        match interfaces {
            Ok(ifaces) => {
                println!("Detected {} interfaces", ifaces.len());
                for iface in ifaces {
                    println!(
                        "  - {}: {} ({:?})",
                        iface.name, iface.ip_address, iface.interface_type
                    );
                    assert!(iface.is_up);
                    assert!(!iface.is_loopback);
                }
            }
            Err(e) => {
                // This is acceptable in some CI/test environments
                println!("Interface detection failed as expected in test env: {}", e);
            }
        }
    }

    #[test]
    fn test_ipconfig_parsing() {
        let sample_output = r#"
Windows IP Configuration

Ethernet adapter Ethernet:

   Connection-specific DNS Suffix  . : my.domain.com
   Description . . . . . . . . . . . : Intel(R) Ethernet Connection (7) I219-LM
   IPv4 Address. . . . . . . . . . . : 192.168.1.100(Preferred)
   Subnet Mask . . . . . . . . . . . : 255.255.255.0
   Default Gateway . . . . . . . . . : 192.168.1.1

Wireless LAN adapter Wi-Fi:

   Media State . . . . . . . . . . . : Media disconnected
   Connection-specific DNS Suffix  . :
   Description . . . . . . . . . . . : Intel(R) Wi-Fi 6 AX201 160MHz

Tunnel adapter Teredo Tunneling Pseudo-Interface:

   Connection-specific DNS Suffix  . :
   Description . . . . . . . . . . . : Teredo Tunneling Pseudo-Interface
   IPv6 Address. . . . . . . . . . . : 2001:0:abcd:ef12:3456:7890:abcd:ef12(Preferred)
   Link-local IPv6 Address . . . . . : fe80::1234:5678:9abc:def0%13(Preferred)
   Default Gateway . . . . . . . . . :

Loopback Pseudo-Interface 1:

   Description . . . . . . . . . . . : Software Loopback Interface 1
   IPv4 Address. . . . . . . . . . . : 127.0.0.1(Preferred)
"#;

        let interfaces = parse_ipconfig_output(sample_output).unwrap();

        // Should detect 1 interface (Ethernet).
        // Wi-Fi is disconnected, Teredo has no IPv4, and Loopback is filtered.
        assert_eq!(interfaces.len(), 1);

        let ethernet = &interfaces[0];
        assert_eq!(ethernet.name, "Ethernet adapter Ethernet");
        assert_eq!(
            ethernet.ip_address,
            "192.168.1.100".parse::<IpAddr>().unwrap()
        );
        assert_eq!(ethernet.interface_type, InterfaceType::Ethernet);
        assert!(ethernet.is_up);
        assert!(!ethernet.is_loopback);
    }

    #[test]
    fn test_windows_metadata() {
        let metadata = gather_windows_metadata();
        assert!(metadata.is_ok());
        let meta = metadata.unwrap();
        assert!(meta.contains_key("platform"));
        assert_eq!(meta.get("platform").unwrap(), "Windows");
    }

    #[test]
    fn test_elevation_check() {
        let requires_admin = requires_elevation(80);
        assert!(requires_admin);

        let no_admin_needed = requires_elevation(8080);
        assert!(!no_admin_needed);
    }
}