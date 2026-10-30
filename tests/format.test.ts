import { describe, expect, it } from 'vitest'
import { formatSize, shortPath } from '../src/lib/format'

describe('formatSize', () => {
  it('handles zero and small values', () => {
    expect(formatSize(0)).toBe('0 B')
    expect(formatSize(512)).toBe('512 B')
  })

  it('scales to MB and GB', () => {
    expect(formatSize(1024)).toBe('1.0 KB')
    expect(formatSize(1024 * 1024 * 3)).toBe('3.0 MB')
    expect(formatSize(1024 * 1024 * 1024 * 5)).toBe('5.0 GB')
  })
})

describe('shortPath', () => {
  it('keeps short paths intact', () => {
    expect(shortPath('C:\\Apps', 20)).toBe('C:\\Apps')
  })

  it('trims long paths in the middle', () => {
    const long = 'C:\\'.padEnd(80, 'x')
    const out = shortPath(long, 20)
    expect(out.length).toBeLessThanOrEqual(20)
    expect(out).toContain('...')
  })
})
