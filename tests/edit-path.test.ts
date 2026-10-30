import { describe, expect, it } from 'vitest'
import { baseName, dirName, isAbsolutePath, isDriveOnly, joinPath } from '../src/lib/path'

// Edit dialog path helpers (REQ-0013 review fix: browse edge cases).
describe('edit dialog path helpers', () => {
  it('splits a windows path into directory and file name', () => {
    expect(dirName('E:\\tools\\Alpha\\Alpha.exe')).toBe('E:\\tools\\Alpha')
    expect(baseName('E:\\tools\\Alpha\\Alpha.exe')).toBe('Alpha.exe')
  })

  it('treats forward slashes and mixed separators the same', () => {
    expect(dirName('Apps/Alpha/Alpha.exe')).toBe('Apps/Alpha')
    expect(baseName('Apps/Alpha/Alpha.exe')).toBe('Alpha.exe')
    expect(dirName('E:/tools\\Alpha/Alpha.exe')).toBe('E:/tools\\Alpha')
  })

  it('has no file name for empty paths, drive roots and trailing separators', () => {
    expect(baseName('')).toBe('')
    expect(baseName('C:\\')).toBe('C:')
    expect(isDriveOnly(baseName('C:\\'))).toBe(true)
    expect(isDriveOnly('D:')).toBe(true)
    expect(isDriveOnly('Alpha.exe')).toBe(false)
    // A drive root has no directory part once separators are stripped.
    expect(dirName('C:\\')).toBe('')
  })

  it('keeps the separator style when joining a directory and a file', () => {
    expect(joinPath('E:\\tools\\Alpha', 'Alpha.exe')).toBe('E:\\tools\\Alpha\\Alpha.exe')
    expect(joinPath('Apps/Alpha', 'Alpha.exe')).toBe('Apps/Alpha/Alpha.exe')
    expect(joinPath('E:\\tools\\Alpha\\', 'Alpha.exe')).toBe('E:\\tools\\Alpha\\Alpha.exe')
  })

  it('returns the directory itself when there is no file name', () => {
    expect(joinPath('E:\\tools\\Alpha', '')).toBe('E:\\tools\\Alpha')
  })

  it('recognises absolute windows paths', () => {
    expect(isAbsolutePath('E:\\tools')).toBe(true)
    expect(isAbsolutePath('\\\\server\\share')).toBe(true)
    expect(isAbsolutePath('Apps/Alpha')).toBe(false)
  })
})
