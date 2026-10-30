# Keep release binaries free of local build paths.
#
# Rust embeds the source location of panic sites and some dependency paths in
# the final executable even with `strip = true`. Remap every machine-specific
# prefix to a neutral build path before invoking cargo/tauri.
function Set-ReproducibleRustFlags {
  $map = [ordered]@{}
  $Root = Split-Path -Parent $PSScriptRoot

  $cargo = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
  $rustup = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE '.rustup' }
  if ($cargo) { $map[$cargo] = 'C:\cargo' }
  if ($rustup) { $map[$rustup] = 'C:\rustup' }
  if ($env:USERPROFILE) { $map[$env:USERPROFILE] = 'C:\user' }
  if ($env:TEMP) { $map[$env:TEMP] = 'C:\temp' }
  $map[$Root] = 'C:\portablehub'

  $flags = @()
  foreach ($from in $map.Keys) {
    if ($from) { $flags += '--remap-path-prefix=' + $from + '=' + $map[$from] }
  }
  if ($env:RUSTFLAGS) { $flags += $env:RUSTFLAGS }
  $env:RUSTFLAGS = ($flags -join ' ')
  Write-Host '[build] local paths remapped for release binary'
}
