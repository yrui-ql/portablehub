#requires -Version 5.1
<#
.SYNOPSIS
  Build PortableHub (frontend + release binary).
#>
param(
  [switch]$SkipFrontend
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ($env:CARGO_HOME) { $env:PATH = "$env:CARGO_HOME\bin;$env:PATH" }
. (Join-Path $PSScriptRoot 'rust-paths.ps1')
Set-ReproducibleRustFlags

if (-not $SkipFrontend) {
  Write-Host '[build] frontend'
  npm run build
  if ($LASTEXITCODE -ne 0) { throw "frontend build failed: $LASTEXITCODE" }
}

Write-Host '[build] tauri release (sets TAURI_ENV_* so assets are embedded)'
npx tauri build --no-bundle
if ($LASTEXITCODE -ne 0) { throw "tauri build failed: $LASTEXITCODE" }

$conf = Get-Content (Join-Path $Root 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$exe = Join-Path $Root ("src-tauri\target\release\{0}.exe" -f $conf.mainBinaryName)
if (-not (Test-Path $exe)) { throw "missing binary: $exe" }

$size = [math]::Round((Get-Item $exe).Length / 1MB, 2)
Write-Host "[build] OK $exe ($size MB)"
