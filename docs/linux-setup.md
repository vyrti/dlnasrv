# OpenDLNA Server - Linux Setup and Configuration Guide

## Installation

### Ubuntu/Debian

#### Option 1: DEB Package (Recommended)

1. **Download and install the DEB package**:
   ```bash
   wget https://github.com/opendlna/opendlna/releases/latest/download/opendlna_amd64.deb
   sudo dpkg -i opendlna_amd64.deb
   sudo apt-get install -f  # Fix any dependency issues
   ```

2. **Enable and start the service**:
   ```bash
   sudo systemctl enable opendlna
   sudo systemctl start opendlna
   ```

#### Option 2: APT Repository

1. **Add the OpenDLNA repository**:
   ```bash
   curl -fsSL https://repo.opendlna.org/gpg | sudo gpg --dearmor -o /usr/share/keyrings/opendlna-archive-keyring.gpg
   echo "deb [signed-by=/usr/share/keyrings/opendlna-archive-keyring.gpg] https://repo.opendlna.org/ubuntu $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/opendlna.list
   ```

2. **Update and install**:
   ```bash
   sudo apt update
   sudo apt install opendlna
   ```

#### Option 3: Snap Package

```bash
sudo snap install opendlna
sudo snap connect opendlna:network-control
sudo snap connect opendlna:removable-media
```

### CentOS/RHEL/Fedora

#### Option 1: RPM Package (Recommended)

1. **Download and install the RPM package**:
   ```bash
   # For CentOS/RHEL 8+
   wget https://github.com/opendlna/opendlna/releases/latest/download/opendlna-x86_64.rpm
   sudo dnf install opendlna-x86_64.rpm
   
   # For CentOS/RHEL 7
   sudo yum install opendlna-x86_64.rpm
   ```

2. **Enable and start the service**:
   ```bash
   sudo systemctl enable opendlna
   sudo systemctl start opendlna
   ```

#### Option 2: YUM/DNF Repository

1. **Add the OpenDLNA repository**:
   ```bash
   sudo tee /etc/yum.repos.d/opendlna.repo << 'EOF'
   [opendlna]
   name=OpenDLNA Repository
   baseurl=https://repo.opendlna.org/centos/$releasever/$basearch/
   enabled=1
   gpgcheck=1
   gpgkey=https://repo.opendlna.org/gpg
   EOF
   ```

2. **Install**:
   ```bash
   # CentOS/RHEL 8+
   sudo dnf install opendlna
   
   # CentOS/RHEL 7
   sudo yum install opendlna
   ```

### Arch Linux

#### Option 1: AUR Package

```bash
# Using yay
yay -S opendlna

# Using paru
paru -S opendlna

# Manual AUR installation
git clone https://aur.archlinux.org/opendlna.git
cd opendlna
makepkg -si
```

#### Option 2: Manual Installation

```bash
wget https://github.com/opendlna/opendlna/releases/latest/download/opendlna-linux-x86_64.tar.gz
tar -xzf opendlna-linux-x86_64.tar.gz
sudo cp opendlna /usr/local/bin/
sudo chmod +x /usr/local/bin/opendlna
```

### openSUSE

#### Option 1: Zypper Repository

```bash
sudo zypper addrepo https://repo.opendlna.org/opensuse/tumbleweed/ opendlna
sudo zypper refresh
sudo zypper install opendlna
```

#### Option 2: RPM Package

```bash
wget https://github.com/opendlna/opendlna/releases/latest/download/opendlna-opensuse-x86_64.rpm
sudo zypper install opendlna-opensuse-x86_64.rpm
```

### Generic Linux (Binary)

For distributions not listed above:

```bash
# Download and install binary
wget https://github.com/opendlna/opendlna/releases/latest/download/opendlna-linux-x86_64.tar.gz
tar -xzf opendlna-linux-x86_64.tar.gz
sudo cp opendlna /usr/local/bin/
sudo chmod +x /usr/local/bin/opendlna

# Create user and directories
sudo useradd -r -s /bin/false opendlna
sudo mkdir -p /etc/opendlna /var/lib/opendlna /var/log/opendlna
sudo chown opendlna:opendlna /var/lib/opendlna /var/log/opendlna
```

## Configuration

### Configuration File Locations

OpenDLNA follows the XDG Base Directory Specification:

- **System-wide config**: `/etc/opendlna/config.toml`
- **User config**: `~/.config/opendlna/config.toml`
- **Database**: `~/.local/share/opendlna/media.db` (user) or `/var/lib/opendlna/media.db` (system)
- **Logs**: `~/.local/share/opendlna/logs/` (user) or `/var/log/opendlna/` (system)

### Default Configuration

```toml
[server]
port = 8080
interface = "0.0.0.0"
name = "OpenDLNA Server"
uuid = "auto-generated-uuid"

[network]
ssdp_port = 1900
interface_selection = "Auto"
multicast_ttl = 4
announce_interval_seconds = 30

[media]
scan_on_startup = true
watch_for_changes = true
supported_extensions = ["mp4", "mkv", "avi", "webm", "mp3", "flac", "wav", "ogg", "jpg", "jpeg", "png", "gif", "webp"]

[[media.directories]]
path = "~/Videos"
recursive = true

[[media.directories]]
path = "~/Music"
recursive = true

[[media.directories]]
path = "~/Pictures"
recursive = true

[database]
path = "~/.local/share/opendlna/media.db"
vacuum_on_startup = false
backup_enabled = true
```

### System-wide Configuration

For system-wide installation, create `/etc/opendlna/config.toml`:

```toml
[server]
port = 8080
interface = "0.0.0.0"
name = "OpenDLNA Server"

[network]
ssdp_port = 1900
interface_selection = "Auto"

[media]
scan_on_startup = true
watch_for_changes = true

[[media.directories]]
path = "/srv/media/videos"
recursive = true

[[media.directories]]
path = "/srv/media/music"
recursive = true

[[media.directories]]
path = "/srv/media/pictures"
recursive = true

[database]
path = "/var/lib/opendlna/media.db"
```

## Systemd Service Configuration

### Service File

The package installation creates `/etc/systemd/system/opendlna.service`:

```ini
[Unit]
Description=OpenDLNA Media Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=opendlna
Group=opendlna
ExecStart=/usr/local/bin/opendlna
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=opendlna

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/opendlna /var/log/opendlna /srv/media
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictNamespaces=true

# Network settings
IPAddressDeny=any
IPAddressAllow=localhost
IPAddressAllow=10.0.0.0/8
IPAddressAllow=172.16.0.0/12
IPAddressAllow=192.168.0.0/16

# Capabilities
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
AmbientCapabilities=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
```

### Service Management

```bash
# Enable service to start at boot
sudo systemctl enable opendlna

# Start service
sudo systemctl start opendlna

# Stop service
sudo systemctl stop opendlna

# Restart service
sudo systemctl restart opendlna

# Check status
sudo systemctl status opendlna

# View logs
sudo journalctl -u opendlna -f

# Reload configuration
sudo systemctl reload opendlna
```

## Security Configuration

### SELinux (CentOS/RHEL/Fedora)

#### Check SELinux Status

```bash
sestatus
getenforce
```

#### Create SELinux Policy

1. **Generate policy from audit logs**:
   ```bash
   # Run OpenDLNA and check for denials
   sudo ausearch -m avc -ts recent | grep opendlna
   
   # Generate policy
   sudo ausearch -m avc -ts recent | grep opendlna | audit2allow -M opendlna_policy
   
   # Install policy
   sudo semodule -i opendlna_policy.pp
   ```

2. **Manual SELinux configuration**:
   ```bash
   # Allow network binding
   sudo setsebool -P httpd_can_network_connect 1
   
   # Allow file access
   sudo semanage fcontext -a -t httpd_exec_t "/usr/local/bin/opendlna"
   sudo semanage fcontext -a -t var_lib_t "/var/lib/opendlna(/.*)?"
   sudo semanage fcontext -a -t var_log_t "/var/log/opendlna(/.*)?"
   sudo restorecon -R /usr/local/bin/opendlna /var/lib/opendlna /var/log/opendlna
   ```

#### Custom SELinux Policy

Create `/etc/selinux/local/opendlna.te`:

```
module opendlna 1.0;

require {
    type unconfined_t;
    type http_port_t;
    type unreserved_port_t;
    class tcp_socket { bind create listen };
    class udp_socket { bind create };
}

# Allow OpenDLNA to bind to HTTP and SSDP ports
allow unconfined_t http_port_t:tcp_socket { bind create listen };
allow unconfined_t unreserved_port_t:udp_socket { bind create };
```

Compile and install:
```bash
checkmodule -M -m -o opendlna.mod opendlna.te
semodule_package -o opendlna.pp -m opendlna.mod
sudo semodule -i opendlna.pp
```

### AppArmor (Ubuntu/Debian)

#### Create AppArmor Profile

Create `/etc/apparmor.d/usr.local.bin.opendlna`:

```
#include <tunables/global>

/usr/local/bin/opendlna {
  #include <abstractions/base>
  #include <abstractions/nameservice>
  #include <abstractions/user-tmp>

  # Binary execution
  /usr/local/bin/opendlna mr,

  # Configuration files
  /etc/opendlna/** r,
  owner @{HOME}/.config/opendlna/** rw,

  # Database and logs
  /var/lib/opendlna/** rw,
  /var/log/opendlna/** rw,
  owner @{HOME}/.local/share/opendlna/** rw,

  # Media directories
  /srv/media/** r,
  owner @{HOME}/Videos/** r,
  owner @{HOME}/Music/** r,
  owner @{HOME}/Pictures/** r,
  /media/** r,
  /mnt/** r,

  # Network access
  network inet stream,
  network inet dgram,
  network inet6 stream,
  network inet6 dgram,

  # System information
  @{PROC}/sys/net/core/somaxconn r,
  @{PROC}/net/if_inet6 r,
  @{PROC}/net/route r,

  # Deny dangerous capabilities
  deny capability sys_admin,
  deny capability sys_ptrace,
  deny @{HOME}/.ssh/** rw,
}
```

Enable the profile:
```bash
sudo apparmor_parser -r /etc/apparmor.d/usr.local.bin.opendlna
sudo aa-enforce /usr/local/bin/opendlna
```

### Firewall Configuration

#### UFW (Ubuntu)

```bash
# Allow OpenDLNA ports
sudo ufw allow 8080/tcp comment 'OpenDLNA HTTP'
sudo ufw allow 1900/udp comment 'OpenDLNA SSDP'

# Allow from local network only
sudo ufw allow from 192.168.0.0/16 to any port 8080
sudo ufw allow from 192.168.0.0/16 to any port 1900

# Enable firewall
sudo ufw enable
```

#### firewalld (CentOS/RHEL/Fedora)

```bash
# Add OpenDLNA service
sudo firewall-cmd --permanent --new-service=opendlna
sudo firewall-cmd --permanent --service=opendlna --set-description="OpenDLNA Media Server"
sudo firewall-cmd --permanent --service=opendlna --add-port=8080/tcp
sudo firewall-cmd --permanent --service=opendlna --add-port=1900/udp

# Enable service
sudo firewall-cmd --permanent --add-service=opendlna
sudo firewall-cmd --reload

# Or add ports directly
sudo firewall-cmd --permanent --add-port=8080/tcp
sudo firewall-cmd --permanent --add-port=1900/udp
sudo firewall-cmd --reload
```

#### iptables (Generic)

```bash
# Allow OpenDLNA ports
sudo iptables -A INPUT -p tcp --dport 8080 -j ACCEPT
sudo iptables -A INPUT -p udp --dport 1900 -j ACCEPT

# Save rules (varies by distribution)
# Ubuntu/Debian:
sudo iptables-save > /etc/iptables/rules.v4

# CentOS/RHEL:
sudo service iptables save
```

## Troubleshooting

### Common Issues

#### 1. Permission Denied Errors

**Symptoms:**
- Cannot bind to port 1900
- Database creation fails
- Cannot access media directories

**Solutions:**

1. **Check user permissions**:
   ```bash
   # Ensure opendlna user exists
   id opendlna
   
   # Fix directory permissions
   sudo chown -R opendlna:opendlna /var/lib/opendlna /var/log/opendlna
   sudo chmod -R 755 /var/lib/opendlna
   sudo chmod -R 644 /var/log/opendlna
   ```

2. **Grant capability to bind privileged ports**:
   ```bash
   sudo setcap 'cap_net_bind_service=+ep' /usr/local/bin/opendlna
   ```

3. **Use systemd socket activation**:
   Create `/etc/systemd/system/opendlna.socket`:
   ```ini
   [Unit]
   Description=OpenDLNA Socket
   
   [Socket]
   ListenStream=8080
   ListenDatagram=1900
   
   [Install]
   WantedBy=sockets.target
   ```

#### 2. SELinux/AppArmor Denials

**Symptoms:**
- Service fails to start
- Network binding errors
- File access denied

**Diagnostics:**
```bash
# SELinux
sudo ausearch -m avc -ts recent | grep opendlna
sudo sealert -a /var/log/audit/audit.log

# AppArmor
sudo dmesg | grep -i apparmor
sudo aa-status
```

**Solutions:**
- Follow the SELinux/AppArmor configuration sections above
- Temporarily disable for testing:
  ```bash
  # SELinux
  sudo setenforce 0
  
  # AppArmor
  sudo aa-disable /usr/local/bin/opendlna
  ```

#### 3. Network Discovery Issues

**Symptoms:**
- DLNA clients cannot find server
- Multicast not working

**Diagnostics:**
```bash
# Check network interfaces
ip addr show
ip route show

# Test multicast
ping -c 3 239.255.255.250

# Check port availability
ss -tulpn | grep -E ':(8080|1900)'
netstat -tulpn | grep -E ':(8080|1900)'

# Check firewall
sudo iptables -L -n
sudo firewall-cmd --list-all
sudo ufw status verbose
```

**Solutions:**

1. **Configure network interface**:
   ```toml
   [network]
   interface_selection = "eth0"  # Specify your interface
   ```

2. **Check multicast routing**:
   ```bash
   # Add multicast route if missing
   sudo ip route add 239.0.0.0/8 dev eth0
   ```

3. **Disable NetworkManager interference**:
   ```bash
   # For specific interface
   sudo nmcli device set eth0 managed no
   ```

#### 4. High CPU/Memory Usage

**Symptoms:**
- High CPU during file scanning
- Excessive memory usage
- System becomes unresponsive

**Solutions:**

1. **Optimize file scanning**:
   ```toml
   [media]
   scan_on_startup = false
   watch_for_changes = true
   
   [[media.directories]]
   path = "/srv/media"
   recursive = true
   exclude_patterns = [".*", "*.tmp", "*.part", "lost+found"]
   ```

2. **Limit systemd resources**:
   Add to service file:
   ```ini
   [Service]
   MemoryLimit=512M
   CPUQuota=50%
   IOWeight=100
   ```

3. **Use ionice and nice**:
   ```bash
   sudo systemctl edit opendlna
   ```
   Add:
   ```ini
   [Service]
   ExecStart=
   ExecStart=/usr/bin/ionice -c 3 /usr/bin/nice -n 10 /usr/local/bin/opendlna
   ```

### Distribution-Specific Issues

#### Ubuntu/Debian

1. **Snap confinement issues**:
   ```bash
   # Connect required interfaces
   sudo snap connect opendlna:network-control
   sudo snap connect opendlna:removable-media
   sudo snap connect opendlna:home
   ```

2. **AppArmor profile conflicts**:
   ```bash
   # Check for conflicting profiles
   sudo aa-status | grep opendlna
   
   # Disable conflicting profiles
   sudo aa-disable /snap/opendlna/current/bin/opendlna
   ```

#### CentOS/RHEL

1. **SELinux boolean settings**:
   ```bash
   sudo setsebool -P httpd_can_network_connect 1
   sudo setsebool -P httpd_use_nfs 1  # For NFS shares
   sudo setsebool -P httpd_use_cifs 1  # For CIFS shares
   ```

2. **Firewalld rich rules**:
   ```bash
   # Allow only from local network
   sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="192.168.0.0/16" service name="opendlna" accept'
   ```

#### Arch Linux

1. **Missing dependencies**:
   ```bash
   # Install optional dependencies
   sudo pacman -S sqlite ffmpeg
   ```

2. **User service instead of system service**:
   ```bash
   # Enable user service
   systemctl --user enable opendlna
   systemctl --user start opendlna
   ```

### Network Troubleshooting

#### Interface Selection

```bash
# List available interfaces
ip link show

# Check interface status
ip addr show eth0

# Test interface connectivity
ping -I eth0 8.8.8.8
```

#### Multicast Testing

```bash
# Join multicast group
echo "Joining multicast group..."
socat UDP4-RECV:1900,ip-add-membership=239.255.255.250:eth0,fork -

# Send multicast packet
echo -e "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nST: upnp:rootdevice\r\nMX: 3\r\n\r\n" | socat - UDP4-DATAGRAM:239.255.255.250:1900,broadcast
```

#### Port Binding Issues

```bash
# Check what's using ports
sudo lsof -i :1900
sudo lsof -i :8080

# Kill conflicting processes
sudo pkill -f "process-name"

# Use alternative ports
# Edit config.toml:
[server]
port = 8081

[network]
ssdp_port = 8082
```

## Advanced Configuration

### Container Deployment

#### Docker

```dockerfile
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    wget \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN wget https://github.com/opendlna/opendlna/releases/latest/download/opendlna-linux-x86_64.tar.gz \
    && tar -xzf opendlna-linux-x86_64.tar.gz \
    && mv opendlna /usr/local/bin/ \
    && chmod +x /usr/local/bin/opendlna

RUN useradd -r -s /bin/false opendlna \
    && mkdir -p /var/lib/opendlna /var/log/opendlna \
    && chown opendlna:opendlna /var/lib/opendlna /var/log/opendlna

USER opendlna
EXPOSE 8080/tcp 1900/udp

CMD ["/usr/local/bin/opendlna"]
```

Run with Docker:
```bash
docker build -t opendlna .
docker run -d \
  --name opendlna \
  --network host \
  -v /path/to/media:/srv/media:ro \
  -v /path/to/config:/etc/opendlna:ro \
  -v opendlna-data:/var/lib/opendlna \
  opendlna
```

#### Docker Compose

```yaml
version: '3.8'

services:
  opendlna:
    build: .
    container_name: opendlna
    network_mode: host
    volumes:
      - /path/to/media:/srv/media:ro
      - ./config.toml:/etc/opendlna/config.toml:ro
      - opendlna-data:/var/lib/opendlna
    restart: unless-stopped
    environment:
      - RUST_LOG=info

volumes:
  opendlna-data:
```

### Performance Tuning

#### Kernel Parameters

Add to `/etc/sysctl.conf`:
```
# Increase network buffers
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216
net.ipv4.udp_rmem_min = 8192
net.ipv4.udp_wmem_min = 8192

# Multicast settings
net.ipv4.igmp_max_memberships = 20
net.ipv4.igmp_max_msf = 10
```

Apply changes:
```bash
sudo sysctl -p
```

#### File System Optimization

For media storage:
```bash
# Mount with optimized options
/dev/sdb1 /srv/media ext4 defaults,noatime,nodiratime 0 2

# For databases (if on separate partition)
/dev/sdc1 /var/lib/opendlna ext4 defaults,noatime 0 2
```

#### Systemd Service Optimization

```ini
[Service]
# Resource limits
MemoryLimit=1G
CPUQuota=200%
IOWeight=500

# Scheduling
Nice=-5
IOSchedulingClass=1
IOSchedulingPriority=4

# Security (relaxed for performance)
PrivateTmp=false
ProtectSystem=false
```

## Logging and Monitoring

### Systemd Journal

```bash
# View logs
sudo journalctl -u opendlna

# Follow logs
sudo journalctl -u opendlna -f

# Filter by priority
sudo journalctl -u opendlna -p err

# Show logs since boot
sudo journalctl -u opendlna -b
```

### Log Rotation

Create `/etc/logrotate.d/opendlna`:
```
/var/log/opendlna/*.log {
    daily
    missingok
    rotate 7
    compress
    delaycompress
    notifempty
    create 644 opendlna opendlna
    postrotate
        systemctl reload opendlna
    endscript
}
```

### Monitoring with Prometheus

Add to OpenDLNA configuration:
```toml
[monitoring]
prometheus_enabled = true
prometheus_port = 9090
metrics_path = "/metrics"
```

### Health Checks

Create health check script `/usr/local/bin/opendlna-health`:
```bash
#!/bin/bash

# Check if service is running
if ! systemctl is-active --quiet opendlna; then
    echo "CRITICAL: OpenDLNA service is not running"
    exit 2
fi

# Check if port is listening
if ! ss -tulpn | grep -q ":8080"; then
    echo "CRITICAL: OpenDLNA is not listening on port 8080"
    exit 2
fi

# Check SSDP port
if ! ss -tulpn | grep -q ":1900"; then
    echo "WARNING: SSDP port 1900 is not available"
    exit 1
fi

echo "OK: OpenDLNA is running and listening on required ports"
exit 0
```

## Getting Help

### System Information Collection

For support requests, collect system information:

```bash
#!/bin/bash
# Create support bundle

mkdir -p opendlna-support
cd opendlna-support

# System information
uname -a > system_info.txt
lsb_release -a >> system_info.txt 2>/dev/null || cat /etc/os-release >> system_info.txt
free -h > memory_info.txt
df -h > disk_info.txt

# Network information
ip addr show > network_interfaces.txt
ip route show > routing_table.txt
ss -tulpn > listening_ports.txt

# OpenDLNA specific
systemctl status opendlna > service_status.txt
journalctl -u opendlna --no-pager > service_logs.txt
cp /etc/opendlna/config.toml config.toml 2>/dev/null || echo "No system config found" > config.toml

# Security information
sestatus > selinux_status.txt 2>/dev/null || echo "SELinux not available" > selinux_status.txt
aa-status > apparmor_status.txt 2>/dev/null || echo "AppArmor not available" > apparmor_status.txt

# Firewall information
iptables -L -n > iptables_rules.txt 2>/dev/null || echo "iptables not available" > iptables_rules.txt
firewall-cmd --list-all > firewalld_rules.txt 2>/dev/null || echo "firewalld not available" > firewalld_rules.txt
ufw status verbose > ufw_status.txt 2>/dev/null || echo "ufw not available" > ufw_status.txt

# Create archive
cd ..
tar -czf opendlna-support-$(date +%Y%m%d-%H%M%S).tar.gz opendlna-support/
echo "Support bundle created: opendlna-support-$(date +%Y%m%d-%H%M%S).tar.gz"
```

### Common Support Information

When reporting issues, include:

1. **Linux distribution and version**
2. **OpenDLNA version** (`opendlna --version`)
3. **Installation method** (package, binary, container)
4. **Configuration file** (remove sensitive information)
5. **Service logs** (`journalctl -u opendlna`)
6. **Network configuration** (`ip addr`, `ip route`)
7. **Firewall status** and rules
8. **SELinux/AppArmor status** if applicable

### Community Resources

- **GitHub Issues**: https://github.com/opendlna/opendlna/issues
- **Documentation**: https://docs.opendlna.org
- **Community Forum**: https://community.opendlna.org
- **IRC**: #opendlna on Libera.Chat
- **Matrix**: #opendlna:matrix.org