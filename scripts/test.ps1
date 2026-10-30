$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ($env:CARGO_HOME) { $env:PATH = "$env:CARGO_HOME\bin;$env:PATH" }

npm test
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo test --manifest-path src-tauri/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
