# WinLLDP - Windows LLDP Service

A Windows implementation of LLDP (Link Layer Discovery Protocol) that helps discover network devices and their capabilities. WinLLDP allows Windows machines to participate in network topology discovery by sending and receiving LLDP packets.

## What is LLDP?

LLDP is a vendor-neutral protocol used by network devices to advertise their identity, capabilities, and neighbors. It's commonly used in enterprise networks for:
- Network topology mapping
- Troubleshooting connectivity
- Automated network documentation
- Switch port identification

## Quick Start for Users

### Download and Install

1. **Download the latest release** from [GitHub Releases](https://github.com/oriolrius/WinLLDP/releases/latest)
   - Download `winlldp-vX.Y.Z.exe` (for 64-bit Windows)
   - No installation needed - it's a standalone executable

2. **Place the executable** in a folder of your choice (e.g., `C:\Tools\WinLLDP\`)

3. **Open PowerShell as Administrator** (required for network access)

### Basic Usage

#### See what devices are on your network:
```powershell
# Start discovering network devices
.\winlldp.exe capture start

# Wait 30-60 seconds for devices to be discovered

# View discovered devices
.\winlldp.exe show-neighbors
```

#### Make your Windows machine discoverable:
```powershell
# Send LLDP announcement once
.\winlldp.exe send

# Or install as a service to send continuously
.\winlldp.exe service install
.\winlldp.exe service start
```

## Installation as a Service (Recommended)

Running WinLLDP as a service ensures your Windows machine is always discoverable on the network.

### Prerequisites

1. **Administrator privileges** (required)
2. **NSSM** - Install via PowerShell:
   ```powershell
   winget install NSSM
   ```

### Install and Start Service

```powershell
# 1. Install the service
.\winlldp.exe service install

# 2. Start the service
.\winlldp.exe service start

# 3. Verify it's running
.\winlldp.exe service status
```

The service will:
- Start automatically with Windows
- Send LLDP packets every 30 seconds
- Continuously discover network neighbors
- Store discoveries in `neighbors.json`

### Verify It's Working

```powershell
# Check discovered devices
.\winlldp.exe show-neighbors

# Watch discoveries in real-time
.\winlldp.exe show-neighbors --watch
```

You should see devices like:
- Network switches with port information
- Other servers running LLDP
- IP phones
- Wireless access points

## Common Commands

### Discovery Commands
```powershell
# Start network discovery
.\winlldp.exe capture start

# View discovered devices
.\winlldp.exe show-neighbors

# Watch discoveries live (updates every 5 seconds)
.\winlldp.exe show-neighbors --watch

# Stop discovery
.\winlldp.exe capture stop
```

### Service Management
```powershell
# Install as Windows service
.\winlldp.exe service install

# Service control
.\winlldp.exe service start
.\winlldp.exe service stop
.\winlldp.exe service restart
.\winlldp.exe service status

# Uninstall service
.\winlldp.exe service uninstall
```

### Troubleshooting Commands
```powershell
# View your network interfaces
.\winlldp.exe show-interfaces

# Send LLDP packet with details
.\winlldp.exe send -v

# Check version
.\winlldp.exe version

# View configuration
.\winlldp.exe show-config
```

## Configuration (Optional)

WinLLDP can be customized using a `.env` file placed in the same directory as `winlldp.exe`. If no `.env` file is provided, WinLLDP uses sensible defaults.

### Configuration Options

#### LLDP_INTERVAL
- **Description**: How often to send LLDP packets (in seconds)
- **Default**: `30`
- **Valid Range**: 5-3600 seconds
- **Example**: `LLDP_INTERVAL=60` (send every minute)
- **Notes**: Lower values increase network traffic but provide faster discovery. Most network devices use 30-60 seconds.

#### LLDP_INTERFACE
- **Description**: Which network interface(s) to use for LLDP
- **Default**: `all` (uses all active network interfaces)
- **Options**:
  - `all` - Use all network interfaces
  - `"Interface Name"` - Use specific interface (use quotes if name has spaces)
- **Examples**:
  ```env
  LLDP_INTERFACE=all
  LLDP_INTERFACE="Ethernet 2"
  LLDP_INTERFACE="Wi-Fi"
  ```
- **Notes**: Use `winlldp.exe show-interfaces` to see available interface names

#### LLDP_SYSTEM_NAME
- **Description**: System name to advertise in LLDP packets
- **Default**: `auto` (uses Windows hostname)
- **Options**:
  - `auto` - Automatically use computer's hostname
  - `"Custom Name"` - Any custom string
- **Examples**:
  ```env
  LLDP_SYSTEM_NAME=auto
  LLDP_SYSTEM_NAME=WebServer01
  LLDP_SYSTEM_NAME="Building 2 - Room 304"
  ```

#### LLDP_SYSTEM_DESCRIPTION
- **Description**: System description in LLDP packets
- **Default**: `Windows LLDP Service`
- **Current Behavior**: ⚠️ **This setting is currently ignored**. WinLLDP always sends detailed Windows information including version, build number, and architecture (e.g., "Windows 10.0.26100 AMD64")
- **Example**: `LLDP_SYSTEM_DESCRIPTION=Database Server` (currently has no effect)

#### LLDP_PORT_DESCRIPTION
- **Description**: Description of the network port
- **Default**: `Ethernet Port`
- **Examples**:
  ```env
  LLDP_PORT_DESCRIPTION=Uplink to Core Switch
  LLDP_PORT_DESCRIPTION=Management Interface
  ```

#### LLDP_MANAGEMENT_ADDRESS
- **Description**: IP address to advertise for management purposes
- **Default**: `auto` (automatically detects primary IP)
- **Options**:
  - `auto` - Use the primary IP of the sending interface
  - `x.x.x.x` - Specific IPv4 address
- **Examples**:
  ```env
  LLDP_MANAGEMENT_ADDRESS=auto
  LLDP_MANAGEMENT_ADDRESS=192.168.1.100
  LLDP_MANAGEMENT_ADDRESS=10.0.0.50
  ```
- **Notes**: This helps network admins know which IP to use for managing the device

#### LLDP_TTL
- **Description**: Time To Live - how long (seconds) receivers should keep this device's information
- **Default**: `120` (2 minutes)
- **Valid Range**: Must be greater than LLDP_INTERVAL and less than 65536
- **Recommended**: 3-4 times the LLDP_INTERVAL value
- **Example**: `LLDP_TTL=180` (3 minutes)
- **Notes**: After TTL expires, network devices will remove this system from their neighbor tables

#### LLDP_NEIGHBORS_FILE
- **Description**: Where to store discovered neighbor information
- **Default**: `neighbors.json` (in the same directory as winlldp.exe)
- **Options**:
  - Relative path: `neighbors.json` (relative to exe location)
  - Absolute path: `C:\ProgramData\WinLLDP\neighbors.json`
- **Examples**:
  ```env
  LLDP_NEIGHBORS_FILE=neighbors.json
  LLDP_NEIGHBORS_FILE=C:\Logs\lldp_neighbors.json
  ```

### Example Configuration Files

#### Minimal Configuration (relies on defaults):
```env
# Empty file - all defaults will be used
```

#### Basic Custom Configuration:
```env
# Send LLDP every minute
LLDP_INTERVAL=60

# Only use the main ethernet interface
LLDP_INTERFACE="Ethernet"

# Custom identity
LLDP_SYSTEM_NAME=FileServer01
```

#### Advanced Configuration:
```env
# LLDP Settings
LLDP_INTERVAL=45
LLDP_INTERFACE="Ethernet 2"
LLDP_SYSTEM_NAME=DC-Primary
LLDP_PORT_DESCRIPTION=Trunk to Core-SW-01 Port Gi1/0/12
LLDP_MANAGEMENT_ADDRESS=10.0.100.15
LLDP_TTL=180

# File Storage
LLDP_NEIGHBORS_FILE=C:\ProgramData\WinLLDP\discovered_devices.json
```

### Default Behavior (No .env File)

If you run WinLLDP without a `.env` file, these defaults are used:

| Setting | Default Value | Behavior |
|---------|--------------|----------|
| LLDP_INTERVAL | 30 | Sends LLDP every 30 seconds |
| LLDP_INTERFACE | all | Uses all network interfaces |
| LLDP_SYSTEM_NAME | auto | Uses computer hostname |
| LLDP_SYSTEM_DESCRIPTION | (ignored) | Sends Windows version info |
| LLDP_PORT_DESCRIPTION | Ethernet Port | Generic port description |
| LLDP_MANAGEMENT_ADDRESS | auto | Uses primary IP address |
| LLDP_TTL | 120 | Neighbors keep info for 2 minutes |
| LLDP_NEIGHBORS_FILE | neighbors.json | Stores in exe directory |

## What You'll See

### Example Output
```
LLDP Neighbors:
+-------------+-----------------+------------------+-----------------+---------+-------+----------------+
| Interface   | Neighbor        | System Name      | Port            | Age     | TTL   | Management IP  |
+=============+=================+==================+=================+=========+=======+================+
| Ethernet 2  | 00:1b:21:XX:XX  | switch-floor-2   | GigabitEthernet | 2m 15s  | 120s  | 192.168.1.10   |
|             |                 |                  | 1/0/24          |         |       |                |
+-------------+-----------------+------------------+-----------------+---------+-------+----------------+
| Ethernet 2  | 00:0c:29:XX:XX  | esxi-host-01     | vmnic2          | 5m 32s  | 120s  | 192.168.1.50   |
+-------------+-----------------+------------------+-----------------+---------+-------+----------------+

Total neighbors: 2
```

## Troubleshooting

### "Access Denied" Error
- **Solution**: Run PowerShell as Administrator
- Right-click PowerShell → "Run as administrator"

### No Devices Discovered
1. **Check Windows Firewall** - LLDP uses a special protocol (not TCP/UDP)
2. **Verify network adapter** - Some virtual adapters don't support LLDP
3. **Wait longer** - Some devices send LLDP every 30-60 seconds
4. **Check switch configuration** - LLDP might be disabled on the switch port

### Service Won't Start
1. **Check if running as admin**
2. **Verify NSSM is installed**: `nssm version`
3. **Check logs**: Look for `nssm_service.log` in the exe directory

## Requirements

- **Windows 10/11 or Windows Server 2016+** (64-bit)
- **Administrator privileges** (for raw network access)
- **NSSM** (for service installation)
- **Network adapter** that supports promiscuous mode

## For Developers

See [Development Guide](docs/development.md) for building from source.

## License

MIT License - See [LICENSE](LICENSE) file for details.


## License

MIT