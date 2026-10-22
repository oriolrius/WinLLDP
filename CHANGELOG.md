# Changelog

All notable changes to WinLLDP are listed here. The file is maintained by [commitizen](https://commitizen-tools.github.io/commitizen/) from the Conventional Commit messages.

## v2.0.0 (2026-10-22)

### BREAKING CHANGE

- WinLLDP is a single native Rust executable (`winlldp.exe`, ~2 MB). Failures exit with code 1,
  the LLDP-MED software revision is "WinLLDP <version>", and the management address TLV carries
  the interface's real ifIndex.

### Feat

- send LLDP through XDP for Windows (AF_XDP API v3); the Microsoft-signed driver package is
  embedded in winlldp.exe and installed transparently on first use
- capture LLDP with Windows Packet Monitor (Windows 11 24H2+ / Windows Server 2025+)
- run the Windows service with the bundled NSSM; uninstalling the service removes the components
  WinLLDP installed
- Hyper-V hosts: send LLDP on the physical adapter behind a vEthernet adapter, so it reaches the
  network, and report its neighbors under the vEthernet name
- `driver status|install|uninstall` to inspect the bundled components

### Fix

- write the application log next to the executable

## v1.0.1 (2026-10-22)

### Fix

- **build**: drop the invalid python-dotenv hidden import
- **lldp**: show IPv6 management addresses in standard notation

## v1.0.0 (2025-07-18)

## v0.10.1 (2025-07-18)

### Feat

- add release script with version bumping and tagging functionality

## v0.10.0 (2025-07-18)

### Feat

- add integration tests for frozen executable and improve README documentation

### Refactor

- centralize path resolution for configuration files and logging

## v0.9.0 (2025-07-18)

### Feat

- add GitHub Actions workflows for building and releasing WinLLDP
- add dynamic version command and bump to v0.9.0

### Fix

- use forward slashes in GitHub release file paths
- pyproject.toml version
- use modern release action with debugging
- add debugging to release workflow
- use modern release action and add write permissions
- install PyInstaller in GitHub Actions workflows

### Refactor

- clean up codebase and remove unused code
- clean up codebase and remove unused code
