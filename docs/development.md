# WinLLDP Development Guide

This guide is for developers who want to build WinLLDP from source or contribute to the project.

## Development Setup

### Prerequisites

- Windows 10/11 or Windows Server
- Python 3.11+
- Git
- Administrator privileges (for testing)

### Clone and Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/oriolrius/WinLLDP.git
   cd WinLLDP
   ```

2. **Install uv package manager**:
   ```powershell
   powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"
   ```

3. **Install dependencies**:
   ```bash
   uv sync
   ```

4. **Copy configuration**:
   ```bash
   copy env.example .env
   ```

### Running from Source

```bash
# Run CLI commands
uv run winlldp --help
uv run winlldp send -v
uv run winlldp capture start

# Or activate venv and run directly
.venv\Scripts\activate
winlldp --help
```

## Building the Executable

### Using Just (Recommended)

```bash
# Build the executable
just pyinstaller

# The executable will be in dist/winlldp.exe
```

### Manual Build

```bash
# Using uv
uv run pyinstaller --clean pyinstaller.spec

# Or with activated venv
.venv\Scripts\activate
pyinstaller --clean pyinstaller.spec
```

## Project Structure

```
winlldp/
├── winlldp/                    # Main Python package
│   ├── __init__.py            # Package init with version
│   ├── cli.py                 # CLI commands using Click
│   ├── config.py              # Configuration management
│   ├── lldp_packet.py         # LLDP packet construction/parsing
│   ├── lldp_receiver.py       # Packet capture and neighbor discovery
│   ├── lldp_sender.py         # LLDP packet transmission
│   ├── logger.py              # Logging functionality
│   ├── paths.py               # Centralized path resolution
│   ├── service_wrapper.py     # Windows service integration
│   └── system_info.py         # System information gathering
├── tests/                     # Test suite
│   ├── test_paths.py          # Path resolution tests
│   ├── test_frozen_executable.py  # Integration tests
│   └── README.md              # Test documentation
├── contrib/                   # Utility scripts
│   └── release.ps1           # Release automation script
├── docs/                      # Documentation
├── dist/                      # Built executables (git ignored)
├── pyproject.toml            # Project configuration
├── pyinstaller.spec          # PyInstaller build spec
├── Justfile                  # Build automation
└── env.example               # Example configuration
```

## Key Components

### Configuration System

- **Priority**: Environment variables → .env file → defaults
- **Path Resolution**: All files stored in exe directory when frozen
- **Auto Values**: `auto` for system name and management IP

### LLDP Implementation

- **IEEE 802.1AB compliant** packet format
- **Multicast address**: 01:80:c2:00:00:0e
- **TLV support**: All mandatory and many optional TLVs

### Service Architecture

```
Windows Service (NSSM)
    └── service_wrapper.py
        ├── LLDPSender (thread) - Sends packets periodically
        └── LLDPReceiver (subprocess) - Captures packets
```

## Testing

### Run All Tests

```bash
# With uv
uv run python -m pytest tests/ -v

# Or with activated venv
python -m pytest tests/ -v
```

### Test Frozen Executable

```bash
# Build and test the executable
python tests/run_frozen_tests.py

# Or manually
just pyinstaller
python -m pytest tests/test_frozen_executable.py -v -s
```

### Important Test Flags

- `-v` : Verbose output
- `-s` : Show print statements (see build progress)
- `--tb=short` : Shorter traceback

## Debugging

### Enable Verbose Logging

Set in `.env`:
```env
LOG_LEVEL=DEBUG
```

Or use `-v` flag:
```bash
winlldp send -v
winlldp capture start -v
```

### Common Issues

1. **Import errors in frozen exe**: Check `pyinstaller.spec` hiddenimports
2. **Path issues**: Use `winlldp.paths` module for all file paths
3. **Service issues**: Check `nssm_service.log` in exe directory

## Contributing

### Code Style

- Follow existing patterns in the codebase
- Use type hints where appropriate
- Add docstrings to new functions/classes
- Keep functions focused and small

### Testing

- Add tests for new features
- Ensure all tests pass before submitting PR
- Test both development and frozen executable

### Commit Messages

Use conventional commits:
- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation
- `test:` Tests
- `refactor:` Code refactoring

### Release Process

1. **Update version** in `pyproject.toml`
2. **Run tests**: `just test` or `python -m pytest`
3. **Build executable**: `just pyinstaller`
4. **Create release**: `.\contrib\release.ps1 -Type minor`

## Advanced Configuration

### Custom TLVs

To add custom TLVs, modify `lldp_packet.py`:

```python
# Add to create_lldp_packet()
custom_tlv = struct.pack('!H', (127 << 9) | len(data)) + data
```

### Interface Filtering

To filter interfaces, modify `system_info.py`:

```python
# In get_interfaces()
if interface_name.startswith('vEthernet'):
    continue  # Skip Hyper-V interfaces
```

### Service Customization

To modify service behavior, edit `service_wrapper.py`:

```python
# Change send interval dynamically
self.sender.interval = new_interval
```

## Resources

- [IEEE 802.1AB-2016 Standard](https://standards.ieee.org/standard/802_1AB-2016.html)
- [Scapy Documentation](https://scapy.readthedocs.io/)
- [PyInstaller Documentation](https://pyinstaller.readthedocs.io/)
- [NSSM Documentation](https://nssm.cc/)