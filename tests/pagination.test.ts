import { describe, expect, it } from 'vitest'
import { PAGE_SIZE, nextPage, pageSlice } from '../src/lib/pagination'

// 10k synthetic entries, matching the scale the render window exists for.
const TEN_K = Array.from({ length: 10_000 }, (_, index) => index)

describe('pageSlice', () => {
  it('renders one 200-card page at the top of the list', () => {
    expect(PAGE_SIZE).toBe(200)
    const page = pageSlice(TEN_K, PAGE_SIZE)
    expect(page).toHaveLength(200)
    expect(page[0]).toBe(0)
    expect(page[199]).toBe(199)
  })

  it('returns the whole list when the limit exceeds its length', () => {
    expect(pageSlice([1, 2, 3], PAGE_SIZE)).toEqual([1, 2, 3])
    expect(pageSlice(TEN_K, TEN_K.length)).toHaveLength(TEN_K.length)
  })

  it('returns an empty slice for a non-positive limit', () => {
    expect(pageSlice(TEN_K, 0)).toEqual([])
    expect(pageSlice(TEN_K, -25)).toEqual([])
  })

  it('truncates fractional limits and never mutates the input', () => {
    const copy = [...TEN_K]
    expect(pageSlice(TEN_K, 2.9)).toEqual([0, 1])
    expect(TEN_K).toEqual(copy)
  })
})

describe('nextPage', () => {
  it('advances by exactly one page size', () => {
    expect(nextPage(PAGE_SIZE)).toBe(PAGE_SIZE * 2)
    expect(nextPage(PAGE_SIZE * 2)).toBe(PAGE_SIZE * 3)
  })

  it('clamps zero and negative limits to the first page', () => {
    expect(nextPage(0)).toBe(PAGE_SIZE)
    expect(nextPage(-10)).toBe(PAGE_SIZE)
  })
})
