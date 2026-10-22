# WinLLDP Development Guide (v2, Rust)

This guide is for developers who want to build WinLLDP from source or contribute to the project.
The Python v1.x line is maintained separately (see [Release lines](#release-lines)).

## Prerequisites

- Rust stable (see `rust-version` in `Cargo.toml`), with `clippy` and `rustfmt`
- [just](install_just.md)
- To run every test: Windows 11 24H2+ (or Server 2025+) and administrator rights (the tests install the bundled XDP driver and the service)

Two ways to build:

- **On Windows**: install Rust with `winget install Rustlang.Rustup`; `just` builds for `x86_64-pc-windows-msvc`.
- **From WSL/Linux**: `rustup target add x86_64-pc-windows-gnu` and `sudo apt install mingw-w64`; `just` cross-compiles for `x86_64-pc-windows-gnu`, and `cargo test` runs the Windows test binaries through WSL interop.

Keep the build directory out of synced folders (OneDrive) with `export CARGO_TARGET_DIR=~/.cache/winlldp-target`.

## Common tasks

```bash
just build    # release build, copied to dist/winlldp.exe
just lint     # cargo fmt --check + clippy -D warnings
just test     # unit tests + integration tests that run winlldp.exe
just bump-dry-run
```

## Project structure

```
src/
├── main.rs      # CLI (clap): every command and option of v1
├── lib.rs       # module list, VERSION, py_str()
├── config.rs    # LLDP_* settings: environment → .env → defaults, validation
├── dotenv.rs    # .env parser compatible with python-dotenv
├── paths.rs     # runtime files live next to the executable
├── packet.rs    # LLDPDU TLV encoding/decoding, to_dict() (neighbors.json fields)
├── sender.rs    # builds each interface's LLDPDU and sends it
├── capture.rs   # capture process (capture-subprocess) and capture start/stop/status
├── xdp.rs       # XDP for Windows: embedded driver package, transparent install/uninstall, AF_XDP send
├── pktmon.rs    # Windows Packet Monitor API (PktMonApi.dll) loaded at run time: capture
├── sysinfo.rs   # interfaces (GetAdaptersAddresses), adapters/Hyper-V uplinks (GetIfTable2), Windows version
├── process.rs   # PID checks, uptime/memory/CPU for capture status
├── service.rs   # service commands through the bundled NSSM, and the service process (service-run)
├── table.rs     # tabulate-style grid tables
├── logger.rs    # winlldp_service.log (next to the exe) + console
└── fsdebug.rs   # verbose file-operation tracing
tests/cli.rs     # integration tests against the real winlldp.exe
```

## Architecture

```
Windows Service (bundled NSSM) → winlldp.exe service-run
    ├── sender thread: LLDP on every active interface every LLDP_INTERVAL seconds, through
    │   XDP for Windows (installed on first use; on Hyper-V, on the physical NIC behind vEthernet)
    └── winlldp.exe capture-subprocess (detached): Windows Packet Monitor, EtherType 0x88cc on
        every active interface; one consumer deduplicates the frames and writes neighbors.json
```

`capture start` spawns the same capture process and waits until it has written `capture.pid`
(written once the capture is open), so failures are reported to the caller.
`cargo run --example pktmon_spike` lists Packet Monitor's data sources and streams LLDP frames,
which helps diagnose interface mapping on other machines.

## Diagrams

The README's diagrams are PlantUML sources in `docs/diagrams/*.puml` (shared style in
`_style.iuml`), rendered to the committed `*.svg` by `just diagrams`
(`scripts/render-diagrams.sh`, PlantUML API server at `PLANTUML_API_BASE`, default
<https://plantuml.joor.net>). Edit the `.puml`, render, and commit both.

## Bundled components

`vendor/xdp-for-windows/<version>/` (signed driver package, unmodified) and `vendor/nssm/<version>/`
are embedded with `include_bytes!` and extracted to `%ProgramFiles%\WinLLDP` when needed. Each folder
has a README with its provenance and `SHA256SUMS`; `THIRD-PARTY-NOTICES.md` carries the licenses.
To update one, replace the files, the sums and the version constant (`xdp::XDP_VERSION`,
`service::NSSM_VERSION`).

## Testing

- Unit tests sit next to the code (`#[cfg(test)]`); `packet.rs` checks the exact bytes v1 sends.
- `tests/cli.rs` copies `winlldp.exe` to a temporary directory and runs it, like v1's
  `test_frozen_executable.py`. Capture tests need the Packet Monitor API; sending needs administrator rights (XDP is installed on the fly); the service test skips without
  administrator rights or NSSM, and when a `WinLLDP` service is already installed.

## Release lines

- **v2.x (Rust)**: this code. `uvx --from commitizen cz bump` bumps `Cargo.toml`/`Cargo.lock` and
  `CHANGELOG.md`, commits and tags `vX.Y.Z`; `git push --follow-tags` runs `release.yml`.
- **v1.x (Python)**: the `v1` branch (pyproject + uv + commitizen), for security and bug fixes. Fixes that affect
  both lines go to both branches.

Use Conventional Commits (`feat:`, `fix:`, `docs:`, ...): commitizen derives the version from them.

## Design decisions

- [ADR 0002: send with bundled XDP for Windows, capture with Packet Monitor, no Npcap](decisions/0002-xdp-and-packet-monitor.md)
- [ADR 0001 (superseded): Npcap research](decisions/0001-npcap.md)

## Resources

- [IEEE 802.1AB-2016](https://standards.ieee.org/standard/802_1AB-2016.html)
- [XDP for Windows](https://github.com/microsoft/xdp-for-windows)
- [Packet Monitor API](https://learn.microsoft.com/en-us/windows/win32/pktmon/pktmon-reference)
- [NSSM](https://nssm.cc/)
