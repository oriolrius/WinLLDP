# WinLLDP - Developer Guide for AI agents

## Project Overview

WinLLDP is a Windows implementation of the Link Layer Discovery Protocol (LLDP). It sends and receives LLDP packets to discover network neighbors and can run as a Windows service (NSSM).

**This line is v2, written in Rust.** It reimplements every v1 (Python) command with the same options, configuration, output and files. The Python v1.x line is still maintained for security and bug fixes on its own branch; keep both compatible (e.g. the `neighbors.json` format).

**Version**: `Cargo.toml` (managed by commitizen, `.cz.toml`)
**Platform**: Windows only (x86_64); built with Rust stable

## Repository Structure

```
src/            # see docs/development.md for the module map
tests/cli.rs    # integration tests that run the real winlldp.exe
Cargo.toml      # dependencies (clap, serde_json, chrono, libloading, ctrlc, windows-sys)
.cz.toml        # commitizen: version_provider = "cargo", tags vX.Y.Z
Justfile        # build / lint / test / bump-dry-run
env.example     # example configuration
```

## Key Components

- **LLDP**: IEEE 802.1AB TLVs (Chassis ID, Port ID, TTL, Port/System Description, System Name, Capabilities, Management Address) plus LLDP-MED (capabilities, hardware/firmware/software revision, model name); multicast 01:80:c2:00:00:0e.
- **Npcap**: `src/pcap.rs` loads `wpcap.dll` at run time. Missing Npcap must always give a clear error, never a fake success.
- **Configuration**: environment → `.env` (python-dotenv compatible parser in `src/dotenv.rs`) → defaults: `LLDP_INTERVAL`, `LLDP_INTERFACE`, `LLDP_TTL`, `LLDP_SYSTEM_NAME`, `LLDP_PORT_DESCRIPTION`, `LLDP_MANAGEMENT_ADDRESS`, `LLDP_NEIGHBORS_FILE`.
- **Files**: next to the executable: `neighbors.json`, `capture.pid`, `winlldp_capture.log`, `nssm_service.log`.

### Build Number Requirement
The Windows build number is sent in LLDP packets: `sysinfo::os_version()` returns e.g. "10.0.26100" (26100 = build), used in the System Description TLV ("Windows 10.0.26100 AMD64") and the LLDP-MED firmware revision.

## Development Workflow

```bash
export CARGO_TARGET_DIR=~/.cache/winlldp-target   # keep builds out of OneDrive
just lint    # cargo fmt --check + clippy -D warnings
just test    # unit + integration tests (needs Npcap/NSSM/admin for the full set)
just build   # dist/winlldp.exe
```

From WSL the build cross-compiles to `x86_64-pc-windows-gnu` and the tests run through WSL interop; on Windows it builds for MSVC (as CI does).

### CLI Commands
`version`, `capture start|stop|status|log`, `show-neighbors [--watch]`, `send [-i IFACE] [-v]`, `show-interfaces`, `show-config`, `debug-paths`, `clear-neighbors`, `service install|uninstall|start|stop|status|restart`; hidden: `capture-subprocess`, `service-run`.

### Code Style Guidelines
- Follow the existing module structure and patterns; keep v1's user-visible output unless a change is deliberate (then document it in README "What changed in v2").
- Errors are reported with a message and exit code 1; network operations must fail gracefully.
- Run `cargo fmt` (max width 120, `rustfmt.toml`).

### Building and releasing
1. Commit with Conventional Commit messages (`feat:`, `fix:`, `docs:`, ...): commitizen derives the version from them
2. `just lint` and `just test`
3. `uvx --from commitizen cz bump` (version in `Cargo.toml` + `CHANGELOG.md`, commit, tag `vX.Y.Z`), then `git push --follow-tags`
4. The tag runs `.github/workflows/release.yml`, which publishes the executable as a GitHub release

### Known Issues
- Requires Administrator privileges (or Npcap without admin-only mode) for raw frames
- Some virtual interfaces may not support LLDP properly
- Neighbors are never expired from `neighbors.json` (same as v1); use `clear-neighbors`
