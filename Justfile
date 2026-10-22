# Justfile for winlldp v2 (Rust)
# On Windows the native toolchain builds for MSVC; from WSL/Linux we cross-compile
# with mingw-w64 (x86_64-pc-windows-gnu) and run the .exe through WSL interop.
set windows-shell := ["powershell.exe", "-NoProfile", "-c"]

target := if os() == "windows" { "x86_64-pc-windows-msvc" } else { "x86_64-pc-windows-gnu" }
target_dir := env_var_or_default("CARGO_TARGET_DIR", "target")

# build the release executable into dist/winlldp.exe
build:
    cargo build --release --target {{target}}
    just _copy-exe

[unix]
_copy-exe:
    mkdir -p dist && cp "{{target_dir}}/{{target}}/release/winlldp.exe" dist/winlldp.exe

[windows]
_copy-exe:
    New-Item -ItemType Directory -Force dist | Out-Null; Copy-Item "{{target_dir}}/{{target}}/release/winlldp.exe" dist/winlldp.exe

# formatting and clippy (warnings are errors)
lint:
    cargo fmt --check
    cargo clippy --target {{target}} --all-targets -- -D warnings

# every test: unit tests and the integration tests that run winlldp.exe, then, one at a time,
# those that remove/reinstall the XDP driver (administrator rights; the service test needs NSSM)
test:
    cargo test --target {{target}}
    cargo test --target {{target}} --test cli -- --ignored --test-threads=1

# preview the next release (version + changelog) from the commit messages
bump-dry-run:
    uvx --from commitizen cz bump --dry-run
