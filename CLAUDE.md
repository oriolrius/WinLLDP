# WinLLDP - Developer Guide for Claude

## Project Overview

WinLLDP is a Python-based Windows implementation of the Link Layer Discovery Protocol (LLDP). It sends and receives LLDP packets to discover network neighbors and can run as a Windows service.

**Project Version**: 0.1.0  
**Python Version**: >=3.11  
**Platform**: Windows (uses pywin32, Windows-specific paths)

## Repository Structure

```
winlldp/
├── winlldp/                    # Main Python package
│   ├── __init__.py            # Package init (version: "0.1.0")
│   ├── cli.py                 # CLI entry point using Click
│   ├── config.py              # Configuration management
│   ├── lldp_packet.py         # LLDP packet construction/parsing
│   ├── lldp_receiver.py       # Packet capture and neighbor discovery
│   ├── lldp_sender.py         # LLDP packet transmission
│   ├── logger.py              # Logging functionality
│   ├── service_wrapper.py     # Windows service integration
│   └── system_info.py         # System information gathering
├── docs/                      # Documentation
│   ├── install_just.md        # Just installation guide
│   └── pyinstaller_notes.md   # PyInstaller usage notes
├── build/                     # PyInstaller build artifacts
├── dist/                      # Compiled executable (winlldp.exe)
├── pyproject.toml            # Project configuration
├── uv.lock                   # Dependency lock file
├── Justfile                  # Build automation
├── *.spec                    # PyInstaller specifications
├── neighbors.json            # Discovered LLDP neighbors
├── nssm_service.log         # Service log file
└── env.example              # Example configuration

```

## Key Components

### 1. **LLDP Implementation**
- **Packet Format**: IEEE 802.1AB compliant with TLV structure
- **Multicast Address**: 01:80:c2:00:00:0e
- **TLVs Supported**: 
  - Mandatory: Chassis ID, Port ID, TTL
  - Optional: Port/System Description, System Name/Capabilities, Management Address
  - LLDP-MED: Hardware/Firmware/Software revision, Model name

### 2. **Service Architecture**
```
Windows Service (NSSM)
    └── service_wrapper.py
        ├── LLDPSender (thread) - Sends packets periodically
        └── LLDPReceiver
            └── Subprocess - Captures packets, writes to neighbors.json
```

### 3. **Configuration**
Priority: Environment Variables → .env file → Defaults
- `LLDP_INTERVAL`: Send interval (default: 30s)
- `LLDP_INTERFACE`: Target interface(s) (default: all)
- `LLDP_TTL`: Time to live (default: 120s)
- `LLDP_MANAGEMENT_ADDRESS`: IP to advertise (default: auto)
- `LLDP_SYSTEM_NAME`: System name (default: hostname)

## Development Workflow

### Setup
```bash
# Install uv package manager
powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"

# Install dependencies
uv sync

# Copy and configure environment
copy env.example .env

# Run CLI
winlldp --help
```

### Build Process
```bash
# Build executable using Just
just pyinstaller

# Or directly with PyInstaller
uvx pyinstaller --log-level DEBUG --clean pyinstaller.spec
```

### CLI Commands

#### Service Management
- `winlldp service install` - Install Windows service
- `winlldp service start/stop/restart` - Control service
- `winlldp service status` - Check status
- `winlldp service uninstall` - Remove service

#### LLDP Operations
- `winlldp send [-v] [-i INTERFACE]` - Send LLDP packet
- `winlldp capture start/stop/status` - Manage capture
- `winlldp show-neighbors [--watch]` - View neighbors
- `winlldp show-interfaces` - List interfaces
- `winlldp show-config` - Display configuration

## Important Development Notes

### ⚠️ Build Number Requirement
**CRITICAL**: Although this is a Python project without config.js files, the Windows build number is dynamically captured and sent in LLDP packets using `platform.version()`. This returns values like "10.0.26100" where 26100 is the Windows build number. This information is included in the system description TLV.

### Code Style Guidelines
- Use existing patterns and conventions in the codebase
- Follow the established module structure
- Maintain clear separation between packet handling, configuration, and UI
- Always handle errors gracefully, especially for network operations
- Use the existing logging infrastructure

### Testing Commands
Currently, there are no automated tests. Manual testing involves:
```bash
# Test sending
winlldp send -v

# Test receiving
winlldp capture start
winlldp show-neighbors --watch

# Test service
winlldp service install
winlldp service start
winlldp service status
```

### Common Tasks

#### Adding New TLVs
1. Update `winlldp/lldp_packet.py` with new TLV types
2. Implement encoding/decoding methods
3. Update sender to include new TLVs
4. Update receiver to parse new TLVs

#### Debugging
- Service logs: `nssm_service.log`
- Capture logs: `winlldp capture log`
- Use `-v` flag for verbose output
- Check Windows Event Log for service issues

#### Building for Distribution
1. Update version in `pyproject.toml` and `__init__.py`
2. Run `just pyinstaller` to create executable
3. Test standalone executable from `dist/`
4. Package with configuration examples

### Dependencies
- **scapy**: Packet manipulation (>=2.5.0)
- **click**: CLI framework (>=8.1.0)
- **python-dotenv**: Configuration (>=1.0.0)
- **pywin32**: Windows integration (>=306)
- **psutil**: System information (>=5.9.0)
- **tabulate**: Table formatting (>=0.9.0)

### Known Issues
- Requires Administrator privileges for raw socket access
- Some virtual interfaces may not support LLDP properly
- Capture subprocess needs proper signal handling

### Future Improvements
- Add automated tests with pytest
- Implement code linting (ruff/flake8)
- Add pre-commit hooks
- Create CI/CD pipeline
- Implement proper versioning strategy
- Add SNMP integration for enterprise environments