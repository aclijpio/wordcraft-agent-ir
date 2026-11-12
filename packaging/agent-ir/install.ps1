#Requires -Version 5.1
param(
    [string]$InstallDir = "",
    [switch]$DesktopShortcut,
    [switch]$UpdateCursorMcp,
    [int]$ControlPort = 17991
)

$ErrorActionPreference = "Stop"
$Source = $PSScriptRoot
if (-not (Test-Path (Join-Path $Source "wordcraft.exe"))) {
    Write-Error "Run this script from the unpacked release folder (wordcraft.exe not found)."
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Programs\WordCraft-Agent-IR"
}

$ProfileDir = Join-Path $InstallDir "profile"
Write-Host "Installing WordCraft Agent IR"
Write-Host "  from: $Source"
Write-Host "  to:   $InstallDir"

New-Item -ItemType Directory -Force -Path $InstallDir, $ProfileDir | Out-Null

$copyNames = @(
    "wordcraft.exe",
    "wordcraft-cli.exe",
    "AGENT-IR.md",
    "README.md",
    "mcp.json.example",
    "docs",
    "skills",
    "templates"
)
foreach ($name in $copyNames) {
    $src = Join-Path $Source $name
    if (-not (Test-Path $src)) { continue }
    $dst = Join-Path $InstallDir $name
    if (Test-Path $src -PathType Container) {
        if (Test-Path $dst) { Remove-Item -Recurse -Force $dst }
        Copy-Item -Recurse -Force $src $dst
    } else {
        Copy-Item -Force $src $dst
    }
}

$startLive = @"
@echo off
set WORDCRAFT_PROFILE_DIR=%~dp0profile
if not exist "%WORDCRAFT_PROFILE_DIR%" mkdir "%WORDCRAFT_PROFILE_DIR%"
start "" "%~dp0wordcraft.exe" --control $ControlPort
"@
Set-Content -Path (Join-Path $InstallDir "start-live.bat") -Value $startLive -Encoding ASCII

$cli = Join-Path $InstallDir "wordcraft-cli.exe"
$gui = Join-Path $InstallDir "wordcraft.exe"
$profileEsc = $ProfileDir.Replace('\', '\\')
$cliEsc = $cli.Replace('\', '\\')

$mcpSnippet = @"
{
  "mcpServers": {
    "wordcraft": {
      "command": "$cliEsc",
      "args": ["mcp"]
    },
    "wordcraft-live": {
      "command": "$cliEsc",
      "args": ["mcp", "--connect", "127.0.0.1:$ControlPort"],
      "env": {
        "WORDCRAFT_PROFILE_DIR": "$profileEsc"
      }
    }
  }
}
"@
Set-Content -Path (Join-Path $InstallDir "cursor-mcp-snippet.json") -Value $mcpSnippet -Encoding UTF8

$programs = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\WordCraft Agent IR"
New-Item -ItemType Directory -Force -Path $programs | Out-Null

$ws = New-Object -ComObject WScript.Shell
$sc1 = $ws.CreateShortcut((Join-Path $programs "WordCraft Agent IR.lnk"))
$sc1.TargetPath = Join-Path $InstallDir "start-live.bat"
$sc1.WorkingDirectory = $InstallDir
$sc1.IconLocation = "$gui,0"
$sc1.Description = "WordCraft Agent IR (live control $ControlPort)"
$sc1.Save()

$sc2 = $ws.CreateShortcut((Join-Path $programs "WordCraft Agent IR CLI.lnk"))
$sc2.TargetPath = $cli
$sc2.Arguments = "commands"
$sc2.WorkingDirectory = $InstallDir
$sc2.IconLocation = "$cli,0"
$sc2.Save()

if ($DesktopShortcut) {
    $desk = [Environment]::GetFolderPath("Desktop")
    $sc3 = $ws.CreateShortcut((Join-Path $desk "WordCraft Agent IR.lnk"))
    $sc3.TargetPath = Join-Path $InstallDir "start-live.bat"
    $sc3.WorkingDirectory = $InstallDir
    $sc3.IconLocation = "$gui,0"
    $sc3.Save()
}

$uninstall = @"
#Requires -Version 5.1
`$ErrorActionPreference = "Stop"
`$InstallDir = "$InstallDir"
`$programs = Join-Path `$env:APPDATA "Microsoft\Windows\Start Menu\Programs\WordCraft Agent IR"
`$desk = [Environment]::GetFolderPath("Desktop")
Get-Process wordcraft -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
if (Test-Path `$programs) { Remove-Item -Recurse -Force `$programs }
`$deskLnk = Join-Path `$desk "WordCraft Agent IR.lnk"
if (Test-Path `$deskLnk) { Remove-Item -Force `$deskLnk }
if (Test-Path `$InstallDir) { Remove-Item -Recurse -Force `$InstallDir }
Write-Host "WordCraft Agent IR removed."
"@
Set-Content -Path (Join-Path $InstallDir "uninstall.ps1") -Value $uninstall -Encoding UTF8

if ($UpdateCursorMcp) {
    $mcpPath = Join-Path $env:USERPROFILE ".cursor\mcp.json"
    if (-not (Test-Path $mcpPath)) {
        $dir = Split-Path $mcpPath
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        Set-Content -Path $mcpPath -Value "{`"mcpServers`":{}}" -Encoding UTF8
    }
    $raw = Get-Content -Raw -Path $mcpPath
    $json = $raw | ConvertFrom-Json
    if (-not $json.mcpServers) {
        $json | Add-Member -NotePropertyName mcpServers -NotePropertyValue ([pscustomobject]@{}) -Force
    }
    $servers = $json.mcpServers
    $wc = [pscustomobject]@{ command = $cli; args = @("mcp") }
    $wcl = [pscustomobject]@{
        command = $cli
        args    = @("mcp", "--connect", "127.0.0.1:$ControlPort")
        env     = [pscustomobject]@{ WORDCRAFT_PROFILE_DIR = $ProfileDir }
    }
    $servers | Add-Member -NotePropertyName wordcraft -NotePropertyValue $wc -Force
    $servers | Add-Member -NotePropertyName wordcraft-live -NotePropertyValue $wcl -Force
    $json | ConvertTo-Json -Depth 20 | Set-Content -Path $mcpPath -Encoding UTF8
    Write-Host "Updated Cursor MCP: $mcpPath"
}

Write-Host ""
Write-Host "Done."
Write-Host "  Start:     $InstallDir\start-live.bat"
Write-Host "  Start Menu: WordCraft Agent IR"
Write-Host "  MCP snippet: $InstallDir\cursor-mcp-snippet.json"
Write-Host "  Uninstall:   powershell -ExecutionPolicy Bypass -File `"$InstallDir\uninstall.ps1`""
if (-not $UpdateCursorMcp) {
    Write-Host "  Tip: re-run with -UpdateCursorMcp to point Cursor at this install."
}
