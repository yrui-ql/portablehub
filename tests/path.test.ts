import { describe, expect, it } from 'vitest'
import { displayPath, isAbsolutePath } from '../src/lib/path'

describe('path helpers', () => {
  it('detects absolute paths', () => {
    expect(isAbsolutePath('C:\\Apps')).toBe(true) // portable:allow fixture
    expect(isAbsolutePath('D:/Tools')).toBe(true) // portable:allow fixture
    expect(isAbsolutePath('\\\\server\\share')).toBe(true)
    expect(isAbsolutePath('Apps')).toBe(false)
  })

  it('renders relative paths against the program folder', () => {
    expect(displayPath('C:\\Program', 'Apps')).toBe('C:\\Program\\Apps') // portable:allow fixture
    expect(displayPath('C:\\Program\\', 'Apps')).toBe('C:\\Program\\Apps') // portable:allow fixture
    expect(displayPath('C:\\Program', '')).toBe('C:\\Program') // portable:allow fixture
  })

  it('keeps absolute paths untouched', () => {
    expect(displayPath('C:\\Program', 'D:/Green/Apps')).toBe('D:\\Green\\Apps') // portable:allow fixture
  })
})
