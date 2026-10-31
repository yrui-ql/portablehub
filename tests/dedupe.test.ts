import { describe, expect, it } from 'vitest'
import { dedupeDiscovered } from '../src/lib/dedupe'
import type { Discovered } from '../src/lib/types'

function item(name: string, path: string): Discovered {
  return { name, path, exe: `${path}/x.exe`, size: 1, kind: 'folder', score: 1 }
}

describe('dedupeDiscovered', () => {
  it('merges same names case-insensitively and keeps the shortest path', () => {
    const result = dedupeDiscovered([
      item('Tool', 'D:/backup/deep/Tool'),
      item('tool', 'E:/apps/Tool'),
      item('Other', 'E:/apps/Other')
    ])
    expect(result).toHaveLength(2)
    const tool = result.find((entry) => entry.name.toLowerCase() === 'tool')
    expect(tool?.path).toBe('E:/apps/Tool')
    expect(tool?.duplicateCount).toBe(1)
  })

  it('keeps single items free of duplicate fields', () => {
    const [only] = dedupeDiscovered([item('Solo', 'E:/apps/Solo')])
    expect(only.duplicateCount).toBeUndefined()
  })

  it('preserves first-seen order after merging', () => {
    const result = dedupeDiscovered([
      item('B', 'E:/b'),
      item('A', 'E:/a'),
      item('B', 'E:/bb')
    ])
    expect(result.map((entry) => entry.name)).toEqual(['B', 'A'])
  })
})
