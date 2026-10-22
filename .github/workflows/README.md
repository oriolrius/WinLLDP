# GitHub Actions workflows (v2, Rust)

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `ci.yml` | push to `master`/`v2`, pull requests | `cargo fmt --check`, `clippy -D warnings`, release build of `winlldp.exe`, all tests (unit and the integration tests that run the executable), uploads the exe as an artifact |
| `release.yml` | push of a `v*` tag | checks the tag matches `Cargo.toml`, lints, builds and tests like CI, takes the release notes from `CHANGELOG.md` and publishes `winlldp-vX.Y.Z.exe` as a GitHub release |

GitHub's Windows runners have no Npcap and no NSSM, so the capture and service tests skip there; the Npcap-missing path is tested instead. Run `just test` on a Windows machine with Npcap, NSSM and administrator rights to run them all.

Dependabot (`../dependabot.yml`) opens weekly grouped updates for the Cargo dependencies (`build(deps): …`) and the actions (`ci: …`).

## Two release lines

- **v1.x** (Python) lives on the `v1` maintenance line and is released with `uv run cz bump`.
- **v2.x** (Rust) is released from this line with commitizen too, reading the version from `Cargo.toml`:

```bash
uvx --from commitizen cz bump --dry-run   # preview the next version and changelog
uvx --from commitizen cz bump             # bump Cargo.toml/Cargo.lock, update CHANGELOG.md, commit, tag vX.Y.Z
git push --follow-tags                    # the tag triggers release.yml
```

A tag runs the `release.yml` of the tagged commit, so v1 and v2 tags each build with their own toolchain.
