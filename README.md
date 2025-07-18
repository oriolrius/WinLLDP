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

Create a `.env` file in the same directory as `winlldp.exe` to customize behavior:

```env
# How often to send LLDP packets (seconds)
LLDP_INTERVAL=30

# Which network interface to use
LLDP_INTERFACE=all              # Use all interfaces
#LLDP_INTERFACE="Ethernet 2"    # Use specific interface

# Custom system name (default: your hostname)
#LLDP_SYSTEM_NAME=MyServer

# Time-to-live for LLDP packets (seconds)
LLDP_TTL=120
```

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