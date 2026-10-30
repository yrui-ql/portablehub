// Fail when a release binary embeds a machine-specific build path.
//
// Rust panics keep source locations even with `strip = true`, so release builds
// must remap cargo/rustup/repo prefixes (see scripts/rust-paths.ps1). This guard
// catches regressions before the binary is published.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const exe = resolve(process.argv[2] || '')
if (!exe) {
  console.error('[release-paths] usage: node scripts/check-release-paths.mjs <exe>')
  process.exit(2)
}

const needles = [
  process.env.USERPROFILE,
  process.env.CARGO_HOME,
  process.env.RUSTUP_HOME,
  process.cwd()
]
  .filter(Boolean)
  .map((path) => path.replace(/[\\/]+$/, ''))
  .filter((path) => /^[A-Za-z]:[\\/]/.test(path))

const bytes = readFileSync(exe)
const ascii = bytes.toString('latin1')
const utf16 = bytes.toString('utf16le')
const hits = []

for (const needle of needles) {
  for (const [label, text] of [['ascii', ascii], ['utf16', utf16]]) {
    if (text.toLowerCase().includes(needle.toLowerCase())) {
      hits.push(`${label}: ${needle}`)
    }
  }
}

if (hits.length) {
  console.error('[release-paths] local build path(s) found in release binary:')
  for (const hit of hits) console.error(`  ${hit}`)
  process.exit(1)
}
console.log(`[release-paths] clean (${needles.length} local prefixes checked)`)
