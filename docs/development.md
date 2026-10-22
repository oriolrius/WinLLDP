# WinLLDP Development Guide (v2, Rust)

This guide is for developers who want to build WinLLDP from source or contribute to the project.
The Python v1.x line is maintained separately (see [Release lines](#release-lines)).

## Prerequisites

- Rust stable (see `rust-version` in `Cargo.toml`), with `clippy` and `rustfmt`
- [just](install_just.md)
- To run every test: Windows with [Npcap](https://npcap.com/), [NSSM](https://nssm.cc/) and administrator rights

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
├── pcap.rs      # Npcap's wpcap.dll loaded at run time: send, filter, capture
├── sysinfo.rs   # interfaces (GetAdaptersAddresses), hostname, Windows version/edition
├── process.rs   # PID checks, uptime/memory/CPU for capture status
├── service.rs   # NSSM service commands and the service process (service-run)
├── table.rs     # tabulate-style grid tables
├── logger.rs    # %TEMP%\winlldp_service.log + console
└── fsdebug.rs   # verbose file-operation tracing
tests/cli.rs     # integration tests against the real winlldp.exe
```

## Architecture

```
Windows Service (NSSM) → winlldp.exe service-run
    ├── sender thread: LLDP on every active interface every LLDP_INTERVAL seconds
    └── winlldp.exe capture-subprocess (detached): one Npcap handle per active
        interface, filter "ether proto 0x88cc", writes neighbors.json
```

`capture start` spawns the same capture process and waits until it has written `capture.pid`
(written once the Npcap handles are open), so failures are reported to the caller.

## Testing

- Unit tests sit next to the code (`#[cfg(test)]`); `packet.rs` checks the exact bytes v1 sends.
- `tests/cli.rs` copies `winlldp.exe` to a temporary directory and runs it, like v1's
  `test_frozen_executable.py`. Capture tests skip without Npcap; the service test skips without
  administrator rights or NSSM, and when a `WinLLDP` service is already installed.

## Release lines

- **v2.x (Rust)**: this code. `uvx --from commitizen cz bump` bumps `Cargo.toml`/`Cargo.lock` and
  `CHANGELOG.md`, commits and tags `vX.Y.Z`; `git push --follow-tags` runs `release.yml`.
- **v1.x (Python)**: the `v1` maintenance line (pyproject + uv + commitizen), for security and bug fixes.

Use Conventional Commits (`feat:`, `fix:`, `docs:`, ...): commitizen derives the version from them.

## Resources

- [IEEE 802.1AB-2016](https://standards.ieee.org/standard/802_1AB-2016.html)
- [Npcap](https://npcap.com/guide/)
- [NSSM](https://nssm.cc/)
