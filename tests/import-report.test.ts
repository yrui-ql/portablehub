import { describe, expect, it } from 'vitest'
import { classifyImportOutcome, summarizeImportReport } from '../src/lib/import-report'
import type { ImportOutcome, ImportReport } from '../src/lib/types'

// Synthetic names/messages only: no real application names belong in this repo.
function outcome(name: string, ok: boolean, message: string): ImportOutcome {
  return { name, ok, message }
}

function report(items: ImportOutcome[], counters = { added: 0, skipped: 0, failed: 0 }): ImportReport {
  return { moved: 0, ...counters, items }
}

describe('classifyImportOutcome', () => {
  it('marks ok outcomes carrying a skip marker as skipped', () => {
    expect(classifyImportOutcome(outcome('Delta', true, '同名应用已存在，跳过'))).toBe('skipped')
    expect(classifyImportOutcome(outcome('Delta', true, '已在列表中，跳过'))).toBe('skipped')
    expect(classifyImportOutcome(outcome('Delta', true, 'already exists'))).toBe('skipped')
    expect(classifyImportOutcome(outcome('Delta', true, 'ALREADY  EXISTS'))).toBe('skipped')
  })

  it('marks ok outcomes without a skip marker as added', () => {
    expect(classifyImportOutcome(outcome('Delta', true, 'link'))).toBe('added')
    expect(classifyImportOutcome(outcome('Delta', true, '已链接并添加'))).toBe('added')
    expect(classifyImportOutcome(outcome('Delta', true, '已移动并添加'))).toBe('added')
  })

  it('does not treat broad words as skip markers', () => {
    // These used to match the old /\bskip\b|pending/ pattern and were therefore
    // misclassified; the pattern must stay narrow.
    expect(classifyImportOutcome(outcome('Delta', true, 'pending review'))).toBe('added')
    expect(classifyImportOutcome(outcome('Delta', true, 'registered, no skip'))).toBe('added')
  })

  it('marks failed outcomes as failed regardless of the message text', () => {
    expect(classifyImportOutcome(outcome('Delta', false, '目标已存在：D:/apps/Delta'))).toBe('failed')
    expect(classifyImportOutcome(outcome('Delta', false, '跳过'))).toBe('failed')
    expect(classifyImportOutcome(outcome('Delta', false, ''))).toBe('failed')
  })
})

describe('summarizeImportReport', () => {
  it('counts added, skipped and failed from the per-item outcomes', () => {
    const summary = summarizeImportReport(
      report([
        outcome('Delta', true, 'link'),
        outcome('Beta', true, 'already exists'),
        outcome('Epsilon', false, 'mock import failed')
      ])
    )

    expect(summary.added).toBe(1)
    expect(summary.skipped).toBe(1)
    expect(summary.failed).toBe(1)
    expect(summary.rows).toEqual([
      { name: 'Delta', message: 'link', state: 'added' },
      { name: 'Beta', message: 'already exists', state: 'skipped' },
      { name: 'Epsilon', message: 'mock import failed', state: 'failed' }
    ])
  })

  it('ignores the backend counters when they disagree (added includes skips)', () => {
    const summary = summarizeImportReport(
      report(
        [outcome('Delta', true, 'link'), outcome('Beta', true, 'already exists')],
        { added: 2, skipped: 1, failed: 0 }
      )
    )

    expect(summary.added).toBe(1)
    expect(summary.skipped).toBe(1)
    expect(summary.failed).toBe(0)
  })

  it('falls back to the report counters when there are no items', () => {
    const summary = summarizeImportReport({
      moved: 0,
      added: 2,
      skipped: 1,
      failed: 3,
      items: []
    })

    expect(summary).toEqual({ added: 2, skipped: 1, failed: 3, rows: [] })
  })
})
