export function isAbsolutePath(value: string): boolean {
  return /^[A-Za-z]:[\\/]/.test(value) || value.startsWith('\\\\') || value.startsWith('/')
}

/** Render a stored path (relative to the program folder, or absolute) for display. */
export function displayPath(root: string, stored: string): string {
  if (!stored) return root
  if (isAbsolutePath(stored)) return stored.replace(/\//g, '\\')
  const base = root.replace(/[\\/]+$/, '')
  return `${base}\\${stored.replace(/\//g, '\\')}`
}

export function dirName(value: string): string {
  const normalized = value.replace(/[\\/]+$/, '')
  const cut = Math.max(normalized.lastIndexOf('\\'), normalized.lastIndexOf('/'))
  return cut > 0 ? normalized.slice(0, cut) : ''
}

export function baseName(value: string): string {
  const normalized = value.replace(/[\\/]+$/, '')
  const cut = Math.max(normalized.lastIndexOf('\\'), normalized.lastIndexOf('/'))
  return cut < 0 ? normalized : normalized.slice(cut + 1)
}

/** A path whose last segment is a drive letter (`C:`) has no file name. */
export function isDriveOnly(value: string): boolean {
  return /^[A-Za-z]:$/.test(value.trim())
}

/**
 * Join a directory with a file name, keeping the separator style of the
 * directory. Returns the directory itself when there is no file name.
 */
export function joinPath(dir: string, file: string): string {
  const base = dir.replace(/[\\/]+$/, '')
  if (!file) return base
  const sep = base.includes('\\') && !base.includes('/') ? '\\' : '/'
  return `${base}${sep}${file}`
}
