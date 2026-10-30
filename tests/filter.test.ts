import { describe, expect, it } from 'vitest'
import { filterApps } from '../src/lib/filter'
import { initialsOf } from '../src/lib/pinyin'
import { fallbackLetter, hashColor } from '../src/lib/theme'

// Fixtures use synthetic names on purpose: no real application names from any
// machine belong in this repository.
const apps = [
  { name: 'Alpha Editor' },
  { name: 'Beta' },
  { name: '示例工具箱' },
  { name: 'Gamma Builder' }
]

// Synthetic Chinese names whose initials are pinned by the tests below:
// 示例工具箱 -> s l g j x, 云端笔记 -> y d b j, 清风听雨 -> q f t y.
const pinyinApps = [
  { name: '示例工具箱' },
  { name: '云端笔记' },
  { name: '清风听雨' },
  { name: '笔记 Pro 2' }
]

describe('initialsOf', () => {
  it('uses the first letter of each hanzi reading', () => {
    expect(initialsOf('示例工具箱')).toBe('slgjx')
    expect(initialsOf('云端笔记')).toBe('ydbj')
    expect(initialsOf('清风听雨')).toBe('qfty')
    expect(initialsOf('蓝')).toBe('l')
  })

  it('keeps ASCII letters and digits, lowercased and in order', () => {
    expect(initialsOf('Alpha Editor')).toBe('alphaeditor')
    expect(initialsOf('笔记 Pro 2')).toBe('bjpro2')
    expect(initialsOf('A1 b2')).toBe('a1b2')
  })

  it('drops whitespace, punctuation and unmapped characters', () => {
    expect(initialsOf('★ 示例 - 工具')).toBe('slgj')
    // U+55E7 is an assigned ideograph with no reading in pinyin-data, so it is
    // skipped while its mapped neighbour still contributes.
    expect(initialsOf('嗧文')).toBe('w')
    expect(initialsOf('嗧')).toBe('')
    expect(initialsOf('   ')).toBe('')
    expect(initialsOf('')).toBe('')
  })
})

describe('filterApps', () => {
  it('returns everything for an empty query', () => {
    expect(filterApps(apps, '')).toHaveLength(4)
    expect(filterApps(apps, '   ')).toHaveLength(4)
    expect(filterApps(pinyinApps, '')).toHaveLength(4)
  })

  it('is case-insensitive', () => {
    expect(filterApps(apps, 'alpha')).toEqual([{ name: 'Alpha Editor' }])
    expect(filterApps(apps, 'ALPHA')).toEqual([{ name: 'Alpha Editor' }])
  })

  it('matches chinese names', () => {
    expect(filterApps(apps, '示例')).toEqual([{ name: '示例工具箱' }])
  })

  it('matches a single pinyin initial', () => {
    expect(filterApps([{ name: '蓝' }, { name: '云' }], 'l')).toEqual([{ name: '蓝' }])
  })

  it('matches multi-character pinyin initials', () => {
    expect(filterApps(pinyinApps, 'slgjx')).toEqual([{ name: '示例工具箱' }])
    expect(filterApps(pinyinApps, 'ydbj')).toEqual([{ name: '云端笔记' }])
    expect(filterApps(pinyinApps, 'qfty')).toEqual([{ name: '清风听雨' }])
  })

  it('matches a substring of the initials, not only the prefix', () => {
    expect(filterApps(pinyinApps, 'gjx')).toEqual([{ name: '示例工具箱' }])
    expect(filterApps(pinyinApps, 'qft')).toEqual([{ name: '清风听雨' }])
  })

  it('matches pinyin initials case-insensitively', () => {
    expect(filterApps(pinyinApps, 'YDBJ')).toEqual([{ name: '云端笔记' }])
    expect(filterApps(pinyinApps, 'QfTy')).toEqual([{ name: '清风听雨' }])
    expect(filterApps(pinyinApps, 'SLGJX')).toEqual([{ name: '示例工具箱' }])
  })

  it('still matches english and chinese name substrings', () => {
    expect(filterApps(apps, 'builder')).toEqual([{ name: 'Gamma Builder' }])
    expect(filterApps(apps, '工具箱')).toEqual([{ name: '示例工具箱' }])
  })

  it('matches mixed english/chinese names by initials', () => {
    expect(filterApps(pinyinApps, 'bjpro2')).toEqual([{ name: '笔记 Pro 2' }])
    expect(filterApps(pinyinApps, 'BJPRO')).toEqual([{ name: '笔记 Pro 2' }])
  })

  it('returns empty when pinyin initials do not match', () => {
    expect(filterApps(pinyinApps, 'xyz')).toEqual([])
    expect(filterApps(pinyinApps, 'slgjy')).toEqual([])
  })

  it('returns empty when nothing matches', () => {
    expect(filterApps(apps, 'zzz')).toEqual([])
  })

  it('filters the virtual autostart tag in both directions', () => {
    const withStarted = [
      { name: 'Start', autostart: true },
      { name: 'Plain', autostart: false }
    ]
    expect(filterApps(withStarted, '', [], [], 'include')).toEqual([{ name: 'Start', autostart: true }])
    expect(filterApps(withStarted, '', [], [], 'exclude')).toEqual([
      { name: 'Plain', autostart: false }
    ])
    expect(filterApps(withStarted, '', [], [], 'off')).toHaveLength(2)
  })
})

describe('theme helpers', () => {
  it('derives a letter fallback', () => {
    expect(fallbackLetter('alpha')).toBe('A')
    expect(fallbackLetter('  示例')).toBe('示')
    expect(fallbackLetter('')).toBe('?')
  })

  it('derives a stable colour', () => {
    expect(hashColor('Beta')).toBe(hashColor('Beta'))
    expect(hashColor('Beta')).not.toBe(hashColor('Alpha'))
  })
})
