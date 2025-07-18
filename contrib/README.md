# WinLLDP Contrib Scripts

This folder contains utility scripts for WinLLDP development and maintenance.

## release.ps1

Automated release script that handles version bumping, committing, tagging, and pushing releases.

### Usage

```powershell
# Bump patch version (e.g., 0.9.0 -> 0.9.1)
.\contrib\release.ps1 -Type patch

# Bump minor version (e.g., 0.9.1 -> 0.10.0)
.\contrib\release.ps1 -Type minor

# Bump major version (e.g., 0.10.0 -> 1.0.0)
.\contrib\release.ps1 -Type major
```

### What it does

1. **Reads current version** from `pyproject.toml`
2. **Calculates new version** based on the type (major/minor/patch)
3. **Updates `pyproject.toml`** with the new version
4. **Commits the change** with message "releasing vX.Y.Z"
5. **Cleans up** any existing local or remote tags with the same name
6. **Creates annotated tag** with message "Release: vX.Y.Z"
7. **Pushes the tag** to origin

### Important Notes

- The script preserves the 'v' prefix if present in the current version
- It automatically cleans up existing tags to avoid conflicts
- After running, you still need to push the commit: `git push origin`
- The tag push will trigger the GitHub Actions release workflow

### Example

```powershell
PS D:\winlldp> .\contrib\release.ps1 -Type minor
Current version: v0.9.9
New version: v0.10.0
Updated pyproject.toml with new version
Committing: releasing v0.10.0
[master 1234567] releasing v0.10.0
 1 file changed, 1 insertion(+), 1 deletion(-)
Cleaning up existing tags...
Creating tag: v0.10.0
Pushing tag to origin...
To github.com:oriolrius/WinLLDP.git
 * [new tag]         v0.10.0 -> v0.10.0

✅ Successfully released v0.10.0

Next steps:
1. Push the commit to origin: git push origin
2. Check GitHub Actions for the automated build
3. Once build completes, the release will be available at:
   https://github.com/oriolrius/WinLLDP/releases/tag/v0.10.0
```