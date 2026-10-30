import {
  PINYIN_INITIALS,
  PINYIN_INITIALS_END,
  PINYIN_INITIALS_START
} from './pinyin-table'

/** Placeholder stored in the table for codepoints without a pinyin reading. */
const NO_INITIAL = ' '

const TABLE_SPAN = PINYIN_INITIALS_END - PINYIN_INITIALS_START + 1

const UPPER_A = 0x41
const UPPER_Z = 0x5a
const LOWER_A = 0x61
const LOWER_Z = 0x7a
const DIGIT_0 = 0x30
const DIGIT_9 = 0x39

/**
 * Reduce text to a lowercase "initials" key used by the search box.
 *
 * Rules (fixed by the tests in tests/filter.test.ts):
 * - a CJK ideograph with a table entry contributes the first letter of its
 *   first pinyin reading (`微` -> `w`)
 * - an ASCII letter or digit is kept and lowercased (`A` -> `a`, `2` -> `2`)
 * - anything else is dropped: whitespace, punctuation, unmapped ideographs
 *
 * Order is preserved, so `initialsOf('笔记 Pro 2')` is `bjpro2`.
 */
export function initialsOf(text: string): string {
  const out: string[] = []
  for (const ch of text) {
    const codepoint = ch.codePointAt(0) ?? 0
    const index = codepoint - PINYIN_INITIALS_START
    if (index >= 0 && index < TABLE_SPAN) {
      const initial = PINYIN_INITIALS[index]
      if (initial !== NO_INITIAL) out.push(initial)
      continue
    }
    if (codepoint >= UPPER_A && codepoint <= UPPER_Z) out.push(ch.toLowerCase())
    else if (
      (codepoint >= LOWER_A && codepoint <= LOWER_Z) ||
      (codepoint >= DIGIT_0 && codepoint <= DIGIT_9)
    ) {
      out.push(ch)
    }
  }
  return out.join('')
}
