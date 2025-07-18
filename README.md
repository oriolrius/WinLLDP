# WinLLDP - Windows LLDP Service

A Python-based LLDP (Link Layer Discovery Protocol) service for Windows that sends and receives LLDP packets to discover network neighbors.

## Features

- Sends LLDP packets periodically with system information
- Discovers and displays LLDP neighbors with persistent storage
- Subprocess-based packet capture for better signal handling (Ctrl+C works!)
- CLI interface for management
- Configurable via .env file
- Can run as a Windows service

## Installation

1. Install uv package manager:
```powershell
powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"
```

2. Install dependencies:
```bash
uv sync
```

## Configuration

Copy `env.example` to `.env` and edit it to configure the service:

```powershell
cp env.example .env  # On Windows, use: copy env.example .env
```

Edit the `.env` file to configure the service:

```env
# LLDP Configuration
LLDP_INTERVAL=30              # Send interval in seconds
LLDP_INTERFACE=all            # Interface to use (all or specific name)
LLDP_SYSTEM_NAME=auto         # System name (auto = hostname)
LLDP_SYSTEM_DESCRIPTION=Windows LLDP Service
LLDP_PORT_DESCRIPTION=Ethernet Port
LLDP_MANAGEMENT_ADDRESS=auto  # Management IP (auto = primary IP)
LLDP_TTL=120                  # Time to live in seconds

# File Configuration
LLDP_NEIGHBORS_FILE=neighbors.json  # Path to neighbors file (relative to project root or absolute)
```

## Usage

### Capture Management (Required First!)

Before viewing neighbors, you must start the capture process:

Start LLDP packet capture (runs in background):
```bash
winlldp capture start
```

Check capture status:
```bash
winlldp capture status
```

View capture log (for debugging):
```bash
winlldp capture log
```

Stop LLDP packet capture:
```bash
winlldp capture stop
```

### Viewing Neighbors

After starting capture, view discovered LLDP neighbors:
```bash
# Show neighbors once
winlldp show-neighbors

# Watch neighbors continuously (updates every 5 seconds)
winlldp show-neighbors --watch
```

Clear all discovered neighbors:
```bash
winlldp clear-neighbors
```

### Sending LLDP Packets

Send LLDP packets immediately:
```bash
winlldp send

# Send with verbose output
winlldp send -v

# Send on specific interface
winlldp send -i "Ethernet 3"
```

### Other Commands

Show network interfaces:
```bash
winlldp show-interfaces
```

Show configuration:
```bash
winlldp show-config
```


## How It Works

1. **Packet Capture**: The receiver runs in a separate subprocess, which allows the main process to remain responsive to Ctrl+C and other signals.

2. **Persistent Storage**: Discovered neighbors are stored in a temporary file, so you can view them even after restarting the CLI.

3. **Background Operation**: The capture process can run in the background - you can start it and close your terminal, then come back later to view neighbors.

## Windows Service Installation

To install as a Windows service (requires Administrator privileges):

### Prerequisites:

1. Install NSSM (if not already installed):
   ```bash
   winget install NSSM
   ```

### Installation:

Install and configure the service with a single command:
```bash
winlldp service install
```

This will automatically:
- Install the WinLLDP service
- Configure it to use the correct Python interpreter from your virtual environment
- Set it to start automatically with Windows
- Configure all necessary paths

### Service Management:

All service operations are integrated into the CLI:

```bash
# Start the service
winlldp service start

# Stop the service
winlldp service stop

# Restart the service
winlldp service restart

# Check service status
winlldp service status

# Uninstall the service
winlldp service uninstall
```

### Logs:

- Service wrapper log: `nssm_service.log` in project directory
- Main service log: `%TEMP%\winlldp_service.log`
- Capture log: `winlldp capture log`

## Requirements

- Windows 10/11 or Windows Server
- Python 3.11+
- Administrator privileges (for raw packet access)
- Npcap or WinPcap (usually installed with Wireshark)

## Troubleshooting

1. **"Access Denied" errors**: Run as Administrator
2. **No packets captured**: Check Windows Firewall settings
3. **Ctrl+C not working**: This should be fixed with the subprocess implementation

## License

MIT