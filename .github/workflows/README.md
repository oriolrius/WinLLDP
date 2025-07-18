# GitHub Actions Workflows

This directory contains GitHub Actions workflows for building and releasing WinLLDP.

## Workflows

### 1. Build and Test (`build.yml`)

- **Triggers**: On push to main/develop branches and pull requests
- **Purpose**: Builds the executable and runs basic tests
- **Artifacts**: Uploads development builds for testing

### 2. Release (`release.yml`)

- **Triggers**: When you push a tag starting with 'v' (e.g., `v0.9.0`)
- **Purpose**: Automatically creates a GitHub release with the built executable
- **Output**: Release with downloadable .exe file

### 3. Manual Release (`manual-release.yml`)

- **Triggers**: Manual workflow dispatch from GitHub Actions tab
- **Purpose**: Create a release with custom version number
- **Features**: 
  - Verifies version matches pyproject.toml
  - Option to mark as pre-release
  - Creates git tag automatically

## How to Create a Release

### Method 1: Push a Tag (Automatic)

```bash
# Make sure version in pyproject.toml is updated
git add pyproject.toml
git commit -m "chore: bump version to 0.9.0"

# Create and push tag
git tag -a v0.9.0 -m "Release v0.9.0"
git push origin v0.9.0
```

### Method 2: Manual Release (GitHub UI)

1. Go to Actions tab in GitHub
2. Select "Manual Release" workflow
3. Click "Run workflow"
4. Enter version number (must match pyproject.toml)
5. Check "pre-release" if applicable
6. Click "Run workflow"

## Release Naming Convention

- Tags: `v0.9.0`, `v1.0.0`, etc.
- Executable: `winlldp-v0.9.0.exe`
- Pre-releases: Same format, marked as pre-release in GitHub

## Notes

- The workflows use `uv` for fast Python dependency management
- PyInstaller is used to create standalone executables
- Releases are automatically created with installation instructions
- Development builds are retained for 7 days
- Release artifacts are retained for 30 days as backup