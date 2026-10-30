// Classification of import outcomes for the result panel (REQ-0014).
// Pure helpers only: no Svelte, no DOM, no I/O, so they can be unit tested.
import type { ImportOutcome, ImportReport } from './types'

export type ImportRowState = 'added' | 'skipped' | 'failed'

export interface ImportReportRow {
  name: string
  message: string
  state: ImportRowState
}

export interface ImportReportSummary {
  added: number
  skipped: number
  failed: number
  rows: ImportReportRow[]
}

// The backend reports "nothing was imported" outcomes as `ok: true` plus a skip
// marker in the message (e.g. "同名应用已存在，跳过"). Keep this deliberately
// narrow: broad words like `skip`/`pending` would misclassify successful
// imports whose mode label or path happens to contain them.
const SKIP_PATTERN = /跳过|已存在|already\s+exists/i

export function classifyImportOutcome(item: ImportOutcome): ImportRowState {
  if (!item.ok) return 'failed'
  return SKIP_PATTERN.test(item.message) ? 'skipped' : 'added'
}

export function summarizeImportReport(report: ImportReport): ImportReportSummary {
  // `items` is authoritative because it is what the panel displays. Only fall
  // back to the backend counters when there are no per-item outcomes.
  if (!report.items.length) {
    return { added: report.added, skipped: report.skipped, failed: report.failed, rows: [] }
  }

  const rows: ImportReportRow[] = report.items.map((item) => ({
    name: item.name,
    message: item.message,
    state: classifyImportOutcome(item)
  }))

  // The Rust command counts every `ok` outcome in `added` (skips included) and
  // counts `skipped` separately, so derive all three counters from the rows
  // rather than trusting the aggregated numbers.
  let added = 0
  let skipped = 0
  let failed = 0
  for (const row of rows) {
    if (row.state === 'failed') failed++
    else if (row.state === 'skipped') skipped++
    else added++
  }

  return { added, skipped, failed, rows }
}
