# WinLLDP Release Script
# Usage: .\release.ps1 -Type [major|minor|patch]
# Example: .\release.ps1 -Type minor

param(
    [Parameter(Mandatory=$true)]
    [ValidateSet("major", "minor", "patch")]
    [string]$Type
)

# Get project root directory
$projectRoot = Split-Path -Parent $PSScriptRoot

# Read current version from pyproject.toml
$pyprojectPath = Join-Path $projectRoot "pyproject.toml"
if (-not (Test-Path $pyprojectPath)) {
    Write-Error "pyproject.toml not found at: $pyprojectPath"
    exit 1
}

# Read the file content
$content = Get-Content $pyprojectPath -Raw

# Extract current version using regex
if ($content -match 'version\s*=\s*"([vV]?)(\d+)\.(\d+)\.(\d+)"') {
    $vPrefix = $matches[1]
    $major = [int]$matches[2]
    $minor = [int]$matches[3]
    $patch = [int]$matches[4]
    
    Write-Host "Current version: $vPrefix$major.$minor.$patch"
} else {
    Write-Error "Could not find version in pyproject.toml"
    exit 1
}

# Calculate new version
switch ($Type) {
    "major" {
        $major++
        $minor = 0
        $patch = 0
    }
    "minor" {
        $minor++
        $patch = 0
    }
    "patch" {
        $patch++
    }
}

$newVersion = "$major.$minor.$patch"
$newVersionWithPrefix = "$vPrefix$newVersion"
Write-Host "New version: $newVersionWithPrefix"

# Update pyproject.toml with new version
$newContent = $content -replace 'version\s*=\s*"[vV]?\d+\.\d+\.\d+"', "version = `"$newVersionWithPrefix`""
Set-Content -Path $pyprojectPath -Value $newContent -NoNewline

Write-Host "Updated pyproject.toml with new version"

# Stage the change
git add $pyprojectPath

# Commit the version bump
$commitMessage = "releasing v$newVersion"
Write-Host "Committing: $commitMessage"
git commit -m $commitMessage

if ($LASTEXITCODE -ne 0) {
    Write-Error "Git commit failed"
    exit 1
}

# Clean up any existing local or remote tags
$tagName = "v$newVersion"
Write-Host "Cleaning up existing tags..."

# Delete remote tag if exists (ignore errors)
git push --delete origin $tagName 2>$null

# Delete local tag if exists (ignore errors)
git tag -d $tagName 2>$null

# Create new annotated tag
Write-Host "Creating tag: $tagName"
git tag -a $tagName -m "Release: v$newVersion"

if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to create tag"
    exit 1
}

# Push the tag to remote
Write-Host "Pushing tag to origin..."
git push origin $tagName

if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to push tag"
    exit 1
}

Write-Host ""
Write-Host "✅ Successfully released v$newVersion" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:"
Write-Host "1. Push the commit to origin: git push origin"
Write-Host "2. Check GitHub Actions for the automated build"
Write-Host "3. Once build completes, the release will be available at:"
Write-Host "   https://github.com/oriolrius/WinLLDP/releases/tag/$tagName"