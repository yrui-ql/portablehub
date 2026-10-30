#requires -Version 5.1
<#
.SYNOPSIS
  Full verification: private terms, unit tests, type check, pinyin table, browser E2E,
  release build, headless selftest. Exit code 0 only when every gate passes.
#>
param(
  [switch]$SkipBuild,
  [switch]$SkipE2E
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ($env:CARGO_HOME) { $env:PATH = "$env:CARGO_HOME\bin;$env:PATH" }
. (Join-Path $PSScriptRoot 'rust-paths.ps1')
Set-ReproducibleRustFlags

function Step([string]$name, [scriptblock]$body) {
  Write-Host "[verify] $name" -ForegroundColor Cyan
  & $body
  if ($LASTEXITCODE -ne 0) { throw "$name failed (exit $LASTEXITCODE)" }
}

Step 'private terms' { node scripts/check-private-terms.mjs }
Step 'frontend unit tests' { npm test }
Step 'frontend type check' { npm run check }
Step 'pinyin initials (offline)' { node scripts/gen-pinyin-initials.mjs --check --offline }
if (-not $SkipE2E) {
  Step 'browser e2e' { npm run e2e }
}
Step 'rust unit tests' { cargo test --manifest-path src-tauri/Cargo.toml }

if (-not $SkipBuild) {
  Step 'release build' { npx tauri build --no-bundle }
}

$conf = Get-Content (Join-Path $Root 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$pkgVersion = (Get-Content (Join-Path $Root 'package.json') -Raw | ConvertFrom-Json).version
$cargoLine = Select-String -Path (Join-Path $Root 'src-tauri\Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
$cargoVersion = $cargoLine.Matches[0].Groups[1].Value
if ($conf.version -ne $pkgVersion -or $conf.version -ne $cargoVersion) {
  throw "version mismatch: tauri=$($conf.version) npm=$pkgVersion cargo=$cargoVersion"
}
$exeName = $conf.mainBinaryName
$exe = Join-Path $Root ("src-tauri\target\release\{0}.exe" -f $exeName)
if (-not (Test-Path $exe)) { throw "missing binary: $exe" }

Write-Host '[verify] release path audit' -ForegroundColor Cyan
node scripts/check-release-paths.mjs $exe
if ($LASTEXITCODE -ne 0) { throw "release path audit failed (exit $LASTEXITCODE)" }

Write-Host '[verify] headless selftest' -ForegroundColor Cyan
$out = Join-Path $env:TEMP "portablehub-selftest-$PID.json"
if (Test-Path $out) { Remove-Item $out -Force }
$p = Start-Process -FilePath $exe -ArgumentList '--selftest', '--out', $out -Wait -PassThru
if (-not (Test-Path $out)) { throw 'selftest produced no output file' }
$json = Get-Content $out -Raw | ConvertFrom-Json
if (-not $json.ok) {
  Write-Host ($json | ConvertTo-Json -Depth 6)
  throw 'selftest reported failures'
}
Write-Host ('[verify] selftest checks: ' + (($json.checks.PSObject.Properties | ForEach-Object { $_.Name }) -join ', '))

# packaging: only one executable in the release directory root
$release = Join-Path $Root 'src-tauri\target\release'
$exes = Get-ChildItem $release -Filter *.exe -File | Where-Object { $_.Name -notmatch '^(build_script_build|.*\.d)$' }
Write-Host ("[verify] release exe: " + ($exes.Name -join ', '))

# autostart must never use the registry
$run = Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -ErrorAction SilentlyContinue
if ($run -and ($run.PSObject.Properties.Name -contains $exeName)) {
  throw "HKCU Run contains $exeName (registry autostart is forbidden)"
}

$sizeMb = [math]::Round((Get-Item $exe).Length / 1MB, 2)
Write-Host "[verify] PASS ($exe, $sizeMb MB)" -ForegroundColor Green
