// Fails when private terms (real app names, machine names, user names) appear in
// the repository. Terms come from the *user* config at runtime and are never
// committed:
//
//   PRIVATE_TERMS="TermA,TermB" node scripts/check-private-terms.mjs
//
// With no terms configured the check is a no-op so the repo stays portable.
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { extname, join, relative } from 'node:path'

const ROOT = process.cwd()
const TERMS = (process.env.PRIVATE_TERMS || '')
  .split(',')
  .map((s) => s.trim())
  .filter(Boolean)

if (!TERMS.length) {
  console.log('[private-terms] no terms configured; skipped')
  process.exit(0)
}

const TEXT_EXT = new Set([
  '.rs', '.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs', '.svelte', '.vue',
  '.json', '.toml', '.ps1', '.psm1', '.sh', '.md', '.yml', '.yaml',
  '.html', '.css', '.txt'
])
const SKIP_DIRS = new Set([
  'node_modules', 'dist', 'build', 'target', '.git', '_cache', 'coverage'
])
const SCAN_PATHS = ['src', 'src-tauri/src', 'tests', 'e2e', 'scripts', 'docs', 'README.md']

function walk(path, acc = []) {
  if (!existsSync(path)) return acc
  const st = statSync(path)
  if (st.isFile()) {
    if (TEXT_EXT.has(extname(path).toLowerCase())) acc.push(path)
    return acc
  }
  for (const name of readdirSync(path)) {
    if (SKIP_DIRS.has(name)) continue
    walk(join(path, name), acc)
  }
  return acc
}

const files = SCAN_PATHS.flatMap((p) => walk(join(ROOT, p)))
const lowerTerms = TERMS.map((t) => t.toLowerCase())
const hits = []

for (const file of files) {
  const lines = readFileSync(file, 'utf8').split(/\r?\n/)
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].toLowerCase()
    for (let t = 0; t < lowerTerms.length; t++) {
      if (line.includes(lowerTerms[t])) {
        hits.push(`${relative(ROOT, file)}:${i + 1}: ${TERMS[t]}`)
      }
    }
  }
}

if (hits.length) {
  console.error(`[private-terms] ${hits.length} hit(s):`)
  for (const hit of hits.slice(0, 50)) console.error(`  ${hit}`)
  process.exit(1)
}

console.log(`[private-terms] clean (${files.length} files, ${TERMS.length} terms)`)
