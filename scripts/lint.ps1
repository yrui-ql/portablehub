$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
npm run check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
