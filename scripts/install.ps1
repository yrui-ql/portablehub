$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
npm ci --no-audit --no-fund
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
