#requires -Version 5.1
<#
.SYNOPSIS
  PortableHub toolchain installer (idempotent).

.DESCRIPTION
  Ensures: MSVC Build Tools, Rust stable-msvc, Node 22.
  Toolchain data lives on E: where possible; MSVC must be installed system-wide.

  Usage:
    powershell -ExecutionPolicy Bypass -File scripts/setup-toolchain.ps1 -Check
    powershell -ExecutionPolicy Bypass -File scripts/setup-toolchain.ps1
#>
param(
  [switch]$Check,
  [ValidateSet('all','msvc','rust','node')]
  [string]$Only = 'all',
  [string]$RustMirror = 'https://rsproxy.cn'
)

$ErrorActionPreference = 'Stop'

$RustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $HOME '.rustup' }
$CargoHome  = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME '.cargo' }
$NpmCache   = if ($env:NPM_CONFIG_CACHE) { $env:NPM_CONFIG_CACHE } else { Join-Path $HOME '.npm' }

$NvmExe = $null
$nvmCmd = Get-Command nvm -ErrorAction SilentlyContinue
if ($nvmCmd) { $NvmExe = $nvmCmd.Source }
elseif ($env:NVM_HOME) { $NvmExe = Join-Path $env:NVM_HOME 'nvm.exe' }
$NvmDir = if ($NvmExe) { Split-Path -Parent $NvmExe } else { $null }

$results = [ordered]@{}

function Write-Step([string]$msg) { Write-Host "[toolchain] $msg" -ForegroundColor Cyan }
function Write-Ok([string]$msg)   { Write-Host "[ok]        $msg" -ForegroundColor Green }
function Write-Miss([string]$msg) { Write-Host "[missing]   $msg" -ForegroundColor Yellow }

function Test-Admin {
  $id = [Security.Principal.WindowsIdentity]::GetCurrent()
  (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-VsWhere {
  $p = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  if (Test-Path $p) { return $p }
  return $null
}

function Test-Msvc {
  $vswhere = Get-VsWhere
  if (-not $vswhere) { return $false }
  $path = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
  return [bool]$path
}

function Test-Rust {
  if (Get-Command rustc -ErrorAction SilentlyContinue) {
    $v = (& rustc -vV 2>$null) -join "`n"
    return ($v -match 'host: x86_64-pc-windows-msvc')
  }
  return $false
}

function Test-Node22 {
  if (-not (Get-Command node -ErrorAction SilentlyContinue)) { return $false }
  $v = (& node -v) -replace '^v',''
  $major = [int]($v.Split('.')[0])
  return ($major -ge 22)
}

# ---------- 1. MSVC Build Tools ----------
$results.msvc = if ($Only -in @('all','msvc')) { Test-Msvc } else { $true }
if (-not $results.msvc) {
  if ($Check) {
    Write-Miss 'MSVC Build Tools (Desktop development with C++)'
  } else {
    if (-not (Test-Admin)) {
      Write-Host 'MSVC Build Tools requires administrator rights.' -ForegroundColor Red
      Write-Host 'Run this script from an elevated PowerShell.' -ForegroundColor Red
      exit 2
    }
    Write-Step 'Installing MSVC Build Tools (4-7 GB, 10-30 min)'
    winget install --source winget --id Microsoft.VisualStudio.2022.BuildTools `
      --accept-package-agreements --accept-source-agreements `
      --override "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100 --add Microsoft.VisualStudio.Component.VC.Redist.14.Latest"
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne -1978335189) {
      throw "MSVC install failed, winget exit code $LASTEXITCODE"
    }
    $results.msvc = Test-Msvc
  }
} else { Write-Ok 'MSVC Build Tools' }

# ---------- 2. Rust ----------
$results.rust = if ($Only -in @('all','rust')) { Test-Rust } else { $true }
if (-not $results.rust) {
  if ($Check) {
    Write-Miss 'Rust stable-x86_64-pc-windows-msvc'
  } else {
    New-Item -ItemType Directory -Force -Path $RustupHome, $CargoHome, $NpmCache | Out-Null
    $env:RUSTUP_HOME = $RustupHome
    $env:CARGO_HOME  = $CargoHome
    $rustupInit = Join-Path $env:TEMP 'rustup-init.exe'
    if (-not (Test-Path $rustupInit)) {
      Write-Step 'Downloading rustup-init.exe'
      Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $rustupInit -UseBasicParsing
    }
    if ($RustMirror) {
      $env:RUSTUP_DIST_SERVER  = $RustMirror
      $env:RUSTUP_UPDATE_ROOT  = "$RustMirror/rustup"
      Write-Step "Using Rust mirror: $RustMirror"
    }
    Write-Step "Installing Rust (RUSTUP_HOME=$RustupHome)"
    & $rustupInit -y --profile minimal --default-toolchain stable-x86_64-pc-windows-msvc --no-modify-path
    if ($LASTEXITCODE -ne 0) { throw "rustup install failed: $LASTEXITCODE" }
    $env:PATH = "$CargoHome\bin;$env:PATH"
    $results.rust = Test-Rust
  }
} else { Write-Ok 'Rust (msvc)' }

# ---------- 3. Node 22 ----------
$results.node = if ($Only -in @('all','node')) { Test-Node22 } else { $true }
if (-not $results.node) {
  if ($Check) {
    Write-Miss 'Node.js 22 (current version is lower)'
  } else {
    if ($NvmExe -and (Test-Path $NvmExe)) {
      $settings = Join-Path $NvmDir 'settings.txt'
      if (Test-Path $settings) {
        $text = Get-Content $settings -Raw
        $text = $text -replace 'https://npm\.taobao\.org/mirrors/node/','https://npmmirror.com/mirrors/node/'
        $text = $text -replace 'https://npm\.taobao\.org/mirrors/npm/','https://npmmirror.com/mirrors/npm/'
        Set-Content -Path $settings -Value $text -Encoding ASCII
      }
      Write-Step 'Installing Node 22 via nvm'
      & $NvmExe install 22
      & $NvmExe use 22
      if ($LASTEXITCODE -ne 0) { throw "nvm install 22 failed: $LASTEXITCODE" }
    } else {
      throw 'nvm not found. Install Node 22 LTS or set NVM_HOME.'
    }
    $results.node = Test-Node22
  }
} else { Write-Ok 'Node 22+' }

# ---------- Summary ----------
if ($Check) {
  $missing = @($results.GetEnumerator() | Where-Object { -not $_.Value } | ForEach-Object { $_.Key })
  if ($missing.Count -gt 0) {
    Write-Host "[toolchain] missing: $($missing -join ', ')" -ForegroundColor Yellow
    exit 1
  }
  Write-Ok 'toolchain complete'
  exit 0
}

$ok = $results.Values -notcontains $false
if (-not $ok) { throw "toolchain installation incomplete" }
Write-Ok 'toolchain ready'
