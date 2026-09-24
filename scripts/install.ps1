# Download the NodKray Windows binary built by GitHub Actions (latest Release).
# Release workflow replaces __BAKE_REPO__ with github.repository.

param(
    [switch]$Uninstall,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

$Repo = if ($env:NODKRAY_REPO) { $env:NODKRAY_REPO } else { "__BAKE_REPO__" }
$Version = if ($env:NODKRAY_VERSION) { $env:NODKRAY_VERSION } else { "latest" }
$Prefix = if ($env:NODKRAY_PREFIX) { $env:NODKRAY_PREFIX } else { Join-Path $env:LOCALAPPDATA "nodkray" }
$BinDir = Join-Path $Prefix "bin"
$Target = Join-Path $BinDir "nodkray.exe"

if ($Help) {
    Write-Host "Usage: install.ps1 [-Uninstall]"
    Write-Host "  NODKRAY_REPO      owner/name (only needed for an unbaked source script)"
    Write-Host "  NODKRAY_VERSION   release tag, or latest"
    Write-Host "  NODKRAY_PREFIX    install prefix (default: %LOCALAPPDATA%\nodkray)"
    exit 0
}

if ($Repo -eq "__BAKE_REPO__" -or $Repo -eq "__REPO__" -or $Repo -eq "OWNER/NodKray") {
    if ($env:GITHUB_REPOSITORY) {
        $Repo = $env:GITHUB_REPOSITORY
    } else {
        Write-Error "Set NODKRAY_REPO=owner/name (example: NODKRAY_REPO=acme/NodKray)"
        exit 2
    }
}

if ($Uninstall) {
    if (Test-Path $Target) {
        Remove-Item -Force $Target
        Write-Host "Removed $Target"
    } else {
        Write-Host "No nodkray binary at $Target"
    }
    Write-Host "Left untouched: MCP servers, Spec-Kit, Herdr, agent configs, and project files."
    Write-Host "For NodKray config and memory: nodkray uninstall"
    Write-Host "For project overlay files:     nodkray uninstall --project"
    exit 0
}

if ($Version -eq "latest") {
    $Url = "https://github.com/$Repo/releases/latest/download/nodkray-x86_64-pc-windows-msvc.zip"
} else {
    $Url = "https://github.com/$Repo/releases/download/$Version/nodkray-x86_64-pc-windows-msvc.zip"
}

$Tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("nodkray-install-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    $Zip = Join-Path $Tmp "nodkray.zip"
    Write-Host "Downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile $Zip
    Expand-Archive -Path $Zip -DestinationPath $Tmp -Force
    $Exe = Get-ChildItem -Path $Tmp -Filter "nodkray.exe" -Recurse | Select-Object -First 1
    if (-not $Exe) {
        Write-Error "Archive did not contain nodkray.exe."
        exit 1
    }
    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
    Copy-Item -Force $Exe.FullName $Target
    Write-Host "Installed $Target"
    & $Target --version
} finally {
    Remove-Item -Recurse -Force $Tmp
}

$PathParts = $env:PATH -split ";"
if ($PathParts -notcontains $BinDir) {
    Write-Host ""
    Write-Host "Add this directory to PATH:"
    Write-Host "  $BinDir"
}
