import { describe, expect, it } from 'vitest'
import { filterApps } from '../src/lib/filter'
import { collectTags, normalizeTags } from '../src/lib/tags'

const apps = [
  { name: 'Alpha', tags: ['工具', '系统'] },
  { name: 'Beta', tags: ['工具'] },
  { name: 'Gamma', tags: [] },
  { name: 'Delta', tags: ['游戏'] }
]

describe('normalizeTags', () => {
  it('trims, de-duplicates case-insensitively and drops empties', () => {
    expect(normalizeTags([' 工具 ', '工具', '', 'Tool', 'tool'])).toEqual(['工具', 'Tool'])
  })

  it('caps tag count and length', () => {
    const many = Array.from({ length: 25 }, (_, i) => `t${i}`)
    expect(normalizeTags(many)).toHaveLength(20)
    expect(normalizeTags(['x'.repeat(25)])).toEqual([])
  })
})

describe('collectTags', () => {
  it('counts usage and sorts by count', () => {
    expect(collectTags(apps)).toEqual([
      { tag: '工具', count: 2 },
      { tag: '系统', count: 1 },
      { tag: '游戏', count: 1 }
    ])
  })

  it('is empty without tags', () => {
    expect(collectTags([{}])).toEqual([])
  })
})

describe('filterApps with tags', () => {
  it('filters by a single tag', () => {
    expect(filterApps(apps, '', ['工具']).map((a) => a.name)).toEqual(['Alpha', 'Beta'])
  })

  it('AND-combines multiple tags', () => {
    expect(filterApps(apps, '', ['工具', '系统']).map((a) => a.name)).toEqual(['Alpha'])
  })

  it('excludes apps carrying an excluded tag', () => {
    expect(filterApps(apps, '', [], ['工具']).map((a) => a.name)).toEqual(['Gamma', 'Delta'])
    expect(filterApps(apps, '', ['工具'], ['系统']).map((a) => a.name)).toEqual(['Beta'])
  })

  it('combines include, exclude and search', () => {
    expect(filterApps(apps, 'a', ['工具'], ['系统']).map((a) => a.name)).toEqual(['Beta'])
  })

  it('matches tag text in search', () => {
    expect(filterApps(apps, '游戏').map((a) => a.name)).toEqual(['Delta'])
  })

  it('returns everything for empty query and tags', () => {
    expect(filterApps(apps, '  ', [])).toHaveLength(4)
  })
})
