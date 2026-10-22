# GitHub Actions workflows (v2, Rust)

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `ci.yml` | push to `master`, pull requests | `cargo fmt --check`, `clippy -D warnings`, release build of `winlldp.exe`, all tests (unit and the integration tests that run the executable), uploads the exe as an artifact |
| `release.yml` | push of a `v*` tag | checks the tag matches `Cargo.toml`, lints, builds and tests like CI, takes the release notes from `CHANGELOG.md` and publishes `winlldp-vX.Y.Z-windows-x64.exe`, `winlldp-vX.Y.Z-windows-x64.zip` (exe, README, LICENSE, notices, changelog, `env.example`), `THIRD-PARTY-NOTICES.md` and `SHA256SUMS.txt` |

The runners are Windows Server 2025 with administrator rights: the tests install the bundled XDP driver, capture with Packet Monitor and run the service with the bundled NSSM; a second step runs the lifecycle tests one at a time.

Dependabot (`../dependabot.yml`, read from the default branch) opens weekly grouped updates for the Cargo dependencies and actions on `master`, and for the uv lockfile and actions on `v1`.

## Two release lines

- **v1.x** (Python) lives on the `v1` branch (security and bug fixes) and is released from there with `uv run cz bump`.
- **v2.x** (Rust) is released from `master` with commitizen too, reading the version from `Cargo.toml`:

```bash
uvx --from commitizen cz bump --dry-run   # preview the next version and changelog
uvx --from commitizen cz bump             # bump Cargo.toml/Cargo.lock, update CHANGELOG.md, commit, tag vX.Y.Z
git push --follow-tags                    # the tag triggers release.yml
```

A tag runs the `release.yml` of the tagged commit, so v1 and v2 tags each build with their own toolchain.
