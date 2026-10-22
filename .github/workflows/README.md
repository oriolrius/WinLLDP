# GitHub Actions workflows

| Workflow | Trigger | What it does |
|----------|---------|--------------|
| `ci.yml` | push to `master`, pull requests | `uv sync --locked`, `ruff check`, builds `winlldp.exe` with PyInstaller, runs all tests (unit, path resolution and the frozen executable), uploads the exe as an artifact |
| `release.yml` | push of a `v*` tag | checks the tag matches `pyproject.toml`, builds and tests like CI, takes the release notes from `CHANGELOG.md` and publishes `winlldp-vX.Y.Z.exe` as a GitHub release |

Dependabot (`../dependabot.yml`) opens weekly grouped updates for the uv lockfile (`build(deps): …`) and the actions (`ci: …`).

## Releasing

Versions and `CHANGELOG.md` are managed with [commitizen](https://commitizen-tools.github.io/commitizen/) from the Conventional Commit messages (`feat:` → minor, `fix:` → patch, `feat!:`/`BREAKING CHANGE` → major):

```bash
uv run cz bump --dry-run     # preview the next version and changelog
uv run cz bump               # bump pyproject.toml, update CHANGELOG.md, commit, tag vX.Y.Z
git push --follow-tags       # the tag triggers release.yml
```
