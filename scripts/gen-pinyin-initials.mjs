// Generates src/lib/pinyin-table.ts: an offline, dependency-free table mapping
// every CJK Unified Ideograph in U+4E00..U+9FFF to the first letter of its first
// pinyin reading. Runtime search reads the table through src/lib/pinyin.ts, so
// the app ships without any pinyin package or network access.
//
// Data: pinyin-data (MIT) https://github.com/mozillazg/pinyin-data
// Line format: `U+4E00: yī,yí,yì  # 一` (first reading first, tone marks added).
//
// Usage:
//   node scripts/gen-pinyin-initials.mjs                    fetch and regenerate
//   node scripts/gen-pinyin-initials.mjs --source <file>    regenerate from a local copy
//   node scripts/gen-pinyin-initials.mjs --check            verify against the upstream source
//   node scripts/gen-pinyin-initials.mjs --check --offline  structural checks only (no upstream compare)
//
// `--check` re-downloads pinyin.txt and compares the mapping against the
// committed artifact: an unreachable source, an unparsable source or any
// mapping drift is a failure (exit 1). Pass `--check --offline` to validate the
// committed artifact on its own (structure, reference initials, size) without
// comparing it against the upstream source; that mode never contacts the network.
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const TABLE_PATH = join(ROOT, 'src', 'lib', 'pinyin-table.ts')
const SOURCE_URL =
  'https://raw.githubusercontent.com/mozillazg/pinyin-data/master/pinyin.txt'
const RANGE_START = 0x4e00
const RANGE_END = 0x9fff
const UNMAPPED = ' '
const MAX_TABLE_BYTES = 60 * 1024
const FETCH_TIMEOUT_MS = 20_000

// Spot checks that stay valid offline. Character -> first letter of its first
// pinyin reading; keeps the generated table honest even without network access.
const REFERENCE_INITIALS = [
  [0x4e00, 'y'], // 一 yi
  [0x4e8c, 'e'], // 二 er
  [0x4e09, 's'], // 三 san
  [0x56db, 's'], // 四 si
  [0x4e94, 'w'], // 五 wu
  [0x516d, 'l'], // 六 liu
  [0x4e03, 'q'], // 七 qi
  [0x516b, 'b'], // 八 ba
  [0x4e5d, 'j'], // 九 jiu
  [0x5341, 's'], // 十 shi
  [0x4e2d, 'z'], // 中 zhong
  [0x56fd, 'g'], // 国 guo
  [0x4eba, 'r'], // 人 ren
  [0x5927, 'd'], // 大 da
  [0x5c0f, 'x'], // 小 xiao
  [0x4e0a, 's'], // 上 shang
  [0x4e0b, 'x'], // 下 xia
  [0x5929, 't'], // 天 tian
  [0x5c71, 's'], // 山 shan
  [0x6c34, 's'], // 水 shui
  [0x706b, 'h'], // 火 huo
  [0x6728, 'm'], // 木 mu
  [0x91d1, 'j'], // 金 jin
  [0x571f, 't'], // 土 tu
  [0x5fae, 'w'], // 微 wei
  [0x4fe1, 'x'], // 信 xin
  [0x5343, 'q'], // 千 qian
  [0x542c, 't'], // 听 ting
  [0x9759, 'j'], // 静 jing
  [0x793a, 's'], // 示 shi
  [0x4f8b, 'l'], // 例 li
  [0x5de5, 'g'], // 工 gong
  [0x5177, 'j'], // 具 ju
  [0x7bb1, 'x'], // 箱 xiang
  [0x4e91, 'y'], // 云 yun
  [0x7aef, 'd'], // 端 duan
  [0x7b14, 'b'], // 笔 bi
  [0x8bb0, 'j'], // 记 ji
  [0x6e05, 'q'], // 清 qing
  [0x98ce, 'f'], // 风 feng
  [0x96e8, 'y'], // 雨 yu
  [0x84dd, 'l'] // 蓝 lan
]

const START_RE = /export const PINYIN_INITIALS_START = 0x([0-9a-fA-F]+)/
const END_RE = /export const PINYIN_INITIALS_END = 0x([0-9a-fA-F]+)/
const DATA_RE = /export const PINYIN_INITIALS =\s*'([^']*)'/
const VERSION_RE = /^\/\/ Source version: (.+)$/m
const HASH_RE = /^\/\/ Source sha256: ([0-9a-f]{64})$/m

const hex = (cp) => `0x${cp.toString(16)}`
const rangeLength = RANGE_END - RANGE_START + 1

const USAGE = `usage: node scripts/gen-pinyin-initials.mjs [--check] [--offline] [--source <pinyin.txt>]

  (default)            fetch pinyin.txt and rewrite src/lib/pinyin-table.ts
  --source <file>      regenerate from a local copy of pinyin.txt
  --check              verify the committed table against the upstream source;
                       an unreachable source or mapping drift exits 1
  --check --offline    validate the committed artifact only; NOT compared with the
                       upstream source (structure, reference initials, size)
  -h, --help           show this help`

function parseArgs(argv) {
  const opts = { check: false, offline: false, sourcePath: null, help: false }
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === '--check') opts.check = true
    else if (arg === '--offline') opts.offline = true
    else if (arg === '--source') {
      opts.sourcePath = argv[++i]
      if (!opts.sourcePath) throw new Error('--source needs a file path')
    } else if (arg === '-h' || arg === '--help') opts.help = true
    else throw new Error(`unknown argument: ${arg}\n\n${USAGE}`)
  }
  return opts
}

function sha256(text) {
  return createHash('sha256').update(text, 'utf8').digest('hex')
}

/** First letter of a pinyin reading, tone marks stripped (`wēi` -> `w`). */
function initialOfSyllable(syllable) {
  const base = syllable
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
  const match = /^[a-z]/.exec(base)
  return match ? match[0] : null
}

async function readSource(opts) {
  if (opts.sourcePath) {
    const file = resolve(opts.sourcePath)
    return { text: readFileSync(file, 'utf8'), label: relative(ROOT, file) }
  }
  const response = await fetch(SOURCE_URL, {
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS)
  })
  if (!response.ok) throw new Error(`HTTP ${response.status} ${response.statusText}`)
  return { text: await response.text(), label: SOURCE_URL }
}

function parseSource(text, label) {
  const initials = new Map()
  const skipped = []
  let version = 'unknown'

  const lines = text.split(/\r?\n/)
  for (let i = 0; i < lines.length; i++) {
    const raw = lines[i].trim()
    if (!raw) continue
    if (raw.startsWith('#')) {
      const versionLine = /^# version:\s*(.+)$/.exec(raw)
      if (versionLine) version = versionLine[1].trim()
      continue
    }

    const parsed = /^U\+([0-9A-Fa-f]{4,6}):\s*([^#]*?)\s*(?:#\s*(.*))?$/.exec(raw)
    if (!parsed) {
      throw new Error(`${label}:${i + 1}: unparseable line: ${raw.slice(0, 80)}`)
    }

    const cp = parseInt(parsed[1], 16)
    if (cp < RANGE_START || cp > RANGE_END) continue

    const firstReading = parsed[2].split(',')[0].trim()
    const initial = initialOfSyllable(firstReading)
    if (!initial) {
      skipped.push({ cp, reading: firstReading })
      continue
    }
    initials.set(cp, initial)
  }

  if (initials.size < 20_000) {
    throw new Error(
      `${label}: only ${initials.size} mappings in U+${hex(RANGE_START)}..U+${hex(RANGE_END)}; source looks incomplete`
    )
  }
  return { initials, skipped, version }
}

function buildData(initials) {
  const chars = []
  for (let cp = RANGE_START; cp <= RANGE_END; cp++) {
    chars.push(initials.get(cp) ?? UNMAPPED)
  }
  return chars.join('')
}

function renderTable(data, meta) {
  return [
    '// AUTO-GENERATED by scripts/gen-pinyin-initials.mjs; do not edit by hand.',
    '// Regenerate: node scripts/gen-pinyin-initials.mjs',
    '//',
    '// Source: pinyin-data (MIT) https://github.com/mozillazg/pinyin-data',
    `// Source version: ${meta.version}`,
    `// Source sha256: ${meta.hash}`,
    '//',
    '// First letter of the first pinyin reading for every CJK Unified Ideograph in',
    '// [PINYIN_INITIALS_START, PINYIN_INITIALS_END]. One character per codepoint:',
    '// an initial (a-z), or a space when the codepoint has no mapping.',
    '// Index into PINYIN_INITIALS with: codepoint - PINYIN_INITIALS_START.',
    '',
    `export const PINYIN_INITIALS_START = ${hex(RANGE_START)}`,
    `export const PINYIN_INITIALS_END = ${hex(RANGE_END)}`,
    '',
    'export const PINYIN_INITIALS =',
    `  '${data}'`,
    ''
  ].join('\n')
}

function parseCommitted(path) {
  let text
  try {
    text = readFileSync(path, 'utf8')
  } catch (err) {
    throw new Error(`cannot read committed table ${relative(ROOT, path)}: ${err.message}`)
  }

  const start = START_RE.exec(text)
  const end = END_RE.exec(text)
  const data = DATA_RE.exec(text)
  if (!start || !end || !data) {
    throw new Error(
      `${relative(ROOT, path)}: missing PINYIN_INITIALS_START / PINYIN_INITIALS_END / PINYIN_INITIALS`
    )
  }

  return {
    text,
    bytes: Buffer.byteLength(text, 'utf8'),
    start: parseInt(start[1], 16),
    end: parseInt(end[1], 16),
    data: data[1],
    version: (VERSION_RE.exec(text) ?? [])[1] ?? null,
    hash: (HASH_RE.exec(text) ?? [])[1] ?? null
  }
}

function countMapped(data) {
  return (data.match(/[a-z]/g) ?? []).length
}

function validateCommitted(table) {
  const problems = []

  if (table.start !== RANGE_START) {
    problems.push(`start is ${hex(table.start)}, expected ${hex(RANGE_START)}`)
  }
  if (table.end !== RANGE_END) {
    problems.push(`end is ${hex(table.end)}, expected ${hex(RANGE_END)}`)
  }
  if (table.data.length !== rangeLength) {
    problems.push(
      `table has ${table.data.length} characters, expected ${rangeLength} (one per codepoint)`
    )
  }
  if (!/^[a-z ]*$/.test(table.data)) {
    const bad = table.data.search(/[^a-z ]/)
    problems.push(
      `table contains a non-initial character at index ${bad} (${JSON.stringify(table.data[bad])})`
    )
  }

  const mapped = countMapped(table.data)
  if (mapped < 20_000) {
    problems.push(`only ${mapped} codepoints are mapped, expected at least 20000`)
  }

  for (const [cp, expected] of REFERENCE_INITIALS) {
    const index = cp - RANGE_START
    const actual = table.data[index]
    if (actual !== expected) {
      problems.push(
        `U+${cp.toString(16)} (${String.fromCodePoint(cp)}) maps to ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`
      )
    }
  }

  if (table.bytes >= MAX_TABLE_BYTES) {
    problems.push(`file is ${table.bytes} bytes, limit is ${MAX_TABLE_BYTES}`)
  }

  return problems
}

function compareMapping(committed, fresh) {
  const diffs = []
  for (let cp = RANGE_START; cp <= RANGE_END; cp++) {
    const have = committed.data[cp - RANGE_START] ?? UNMAPPED
    const want = fresh.initials.get(cp) ?? UNMAPPED
    if (have !== want) diffs.push({ cp, have, want })
  }
  return diffs
}

function describeSkipped(skipped) {
  return skipped
    .slice(0, 5)
    .map((item) => `U+${item.cp.toString(16)} (${item.reading})`)
    .join(', ')
}

async function generate(opts) {
  const source = await readSource(opts)
  const parsed = parseSource(source.text, source.label)
  const meta = { version: parsed.version, hash: sha256(source.text) }
  const data = buildData(parsed.initials)
  const text = renderTable(data, meta)
  const bytes = Buffer.byteLength(text, 'utf8')

  if (bytes >= MAX_TABLE_BYTES) {
    throw new Error(`rendered table is ${bytes} bytes, limit is ${MAX_TABLE_BYTES}`)
  }

  writeFileSync(TABLE_PATH, text)
  const mapped = countMapped(data)
  console.log(`[pinyin-table] wrote ${relative(ROOT, TABLE_PATH)}`)
  console.log(`[pinyin-table]   source: ${source.label} (v${parsed.version})`)
  console.log(`[pinyin-table]   range: U+${RANGE_START.toString(16)}..U+${RANGE_END.toString(16)} (${data.length} codepoints)`)
  console.log(`[pinyin-table]   mapped: ${mapped}, unmapped: ${data.length - mapped}`)
  console.log(`[pinyin-table]   size: ${bytes} bytes (limit ${MAX_TABLE_BYTES})`)
  if (parsed.skipped.length) {
    console.warn(
      `[pinyin-table] warning: skipped ${parsed.skipped.length} reading(s) without a latin initial: ${describeSkipped(parsed.skipped)}`
    )
  }
}

function fail(lines) {
  console.error('[pinyin-table] failed: committed artifact did not pass verification')
  for (const line of lines) console.error(`  - ${line}`)
  process.exitCode = 1
}

async function check(opts) {
  let table
  try {
    table = parseCommitted(TABLE_PATH)
  } catch (err) {
    fail([err.message])
    return
  }

  const problems = validateCommitted(table)
  if (problems.length) {
    fail(problems)
    return
  }

  const mapped = countMapped(table.data)
  console.log(
    `[pinyin-table] integrity OK: ${table.data.length} codepoints, ${mapped} mapped, ${table.bytes} bytes (limit ${MAX_TABLE_BYTES})`
  )
  console.log(
    `[pinyin-table] reference initials OK: ${REFERENCE_INITIALS.length} spot checks`
  )

  if (opts.offline) {
    console.log(
      '[pinyin-table] offline-structural-only: committed artifact is structurally valid; NOT compared with the upstream source'
    )
    return
  }

  let source
  let parsed
  try {
    source = await readSource(opts)
    parsed = parseSource(source.text, source.label)
  } catch (err) {
    const reason = opts.sourcePath
      ? 'source file unavailable'
      : 'source unavailable (network offline?)'
    console.error(`[pinyin-table] failed: ${reason}: ${err.message}`)
    console.error(
      '[pinyin-table] hint: use --check --offline to validate the committed artifact without an upstream compare'
    )
    process.exitCode = 1
    return
  }

  const diffs = compareMapping(table, parsed)
  if (diffs.length) {
    console.error(
      `[pinyin-table] failed: committed table differs from the regenerated mapping (${diffs.length} codepoint(s))`
    )
    for (const diff of diffs.slice(0, 10)) {
      console.error(
        `  - U+${diff.cp.toString(16)} (${String.fromCodePoint(diff.cp)}): committed ${JSON.stringify(diff.have)}, source ${JSON.stringify(diff.want)}`
      )
    }
    if (diffs.length > 10) console.error(`  - ... and ${diffs.length - 10} more`)
    console.error('[pinyin-table] run: node scripts/gen-pinyin-initials.mjs')
    process.exitCode = 1
    return
  }

  const freshHash = sha256(source.text)
  if (table.hash && table.hash !== freshHash) {
    console.log(
      `[pinyin-table] note: committed source sha256 ${table.hash.slice(0, 12)}... differs from current ${freshHash.slice(0, 12)}...; mapping is identical, no regeneration needed`
    )
  }
  if (table.version && parsed.version !== table.version) {
    console.log(
      `[pinyin-table] note: committed source version ${table.version}, current upstream ${parsed.version}; mapping is identical`
    )
  }
  console.log(
    '[pinyin-table] compared-with-source: committed table matches the pinyin-data source'
  )
}

async function main() {
  const opts = parseArgs(process.argv.slice(2))
  if (opts.help) {
    console.log(USAGE)
    return
  }
  if (opts.check) await check(opts)
  else await generate(opts)
}

main().catch((err) => {
  console.error(`[pinyin-table] ERROR ${err.message}`)
  process.exitCode = 1
})
