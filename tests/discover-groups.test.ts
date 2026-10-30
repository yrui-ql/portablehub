import { describe, expect, it } from 'vitest'
import { groupDiscovered, sourceDirOf } from '../src/lib/discover-groups'
import type { Discovered } from '../src/lib/types'

// Synthetic names/paths only: no real application names belong in this repo.
function app(name: string, path: string, kind: 'folder' | 'file' = 'folder'): Discovered {
  return {
    name,
    path,
    exe: kind === 'folder' ? `${path}/${name}.exe` : path,
    size: 1024 * 1024,
    kind,
    score: 500
  }
}

describe('sourceDirOf', () => {
  it('returns the parent directory normalized to forward slashes', () => {
    expect(sourceDirOf('E:/apps/A/One')).toBe('E:/apps/A')
    expect(sourceDirOf('E:\\apps\\A\\One')).toBe('E:/apps/A')
    expect(sourceDirOf('E:/apps/A/One/')).toBe('E:/apps/A')
    expect(sourceDirOf('E:/Delta')).toBe('E:')
  })

  it('returns an empty key when there is no parent directory', () => {
    expect(sourceDirOf('')).toBe('')
    expect(sourceDirOf('Delta')).toBe('')
  })
})

describe('groupDiscovered', () => {
  it('merges items from the same directory and sorts them by name', () => {
    const groups = groupDiscovered([
      app('Two', 'E:/apps/A/Two'),
      app('One', 'E:/apps/A/One'),
      app('Three', 'E:/apps/A/Three')
    ])

    expect(groups).toHaveLength(1)
    expect(groups[0]).toMatchObject({
      key: 'E:/apps/A',
      label: 'E:\\apps\\A',
      hint: 'E:\\apps\\A'
    })
    expect(groups[0].items.map((item) => item.name)).toEqual(['One', 'Three', 'Two'])
  })

  it('sorts groups by directory and parks unknown sources last', () => {
    const groups = groupDiscovered([
      app('Zulu', 'E:/z/Zulu'),
      app('Yankee', 'D:/a/Yankee'),
      app('Loose', 'Loose')
    ])

    expect(groups.map((group) => group.key)).toEqual(['D:/a', 'E:/z', ''])
    expect(groups[2].label).toBe('?')
    expect(groups[2].hint).toBe('')
    expect(groups[2].items.map((item) => item.name)).toEqual(['Loose'])
  })

  it('sorts names case-insensitively inside a group', () => {
    const groups = groupDiscovered([
      app('beta', 'E:/apps/A/beta'),
      app('Alpha', 'E:/apps/A/Alpha'),
      app('Gamma', 'E:/apps/A/Gamma')
    ])

    expect(groups[0].items.map((item) => item.name)).toEqual(['Alpha', 'beta', 'Gamma'])
  })

  it('merges a single file and a folder sharing the same parent directory', () => {
    const groups = groupDiscovered([
      app('Tool', 'E:/apps/A/Tool.exe', 'file'),
      app('ToolFolder', 'E:/apps/A/ToolFolder')
    ])

    expect(groups).toHaveLength(1)
    expect(groups[0].key).toBe('E:/apps/A')
    expect(groups[0].items.map((item) => item.kind)).toEqual(['file', 'folder'])
  })

  it('shortens labels for deep directories but keeps the full hint', () => {
    const groups = groupDiscovered([app('Deep', 'E:/apps/A/deep/Deep')])

    expect(groups[0].key).toBe('E:/apps/A/deep')
    expect(groups[0].label).toBe('E:\\…\\A\\deep')
    expect(groups[0].hint).toBe('E:\\apps\\A\\deep')
  })

  it('treats forward and backslash paths as the same group', () => {
    const groups = groupDiscovered([
      app('One', 'E:/apps/A/One'),
      app('Two', 'E:\\apps\\A\\Two')
    ])

    expect(groups).toHaveLength(1)
    expect(groups[0].items).toHaveLength(2)
  })

  it('returns no groups for an empty result set', () => {
    expect(groupDiscovered([])).toEqual([])
  })
})
