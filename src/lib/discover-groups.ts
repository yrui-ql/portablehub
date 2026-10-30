// Group discovered apps by their source directory (REQ-0021).
// Pure helpers only: no Svelte, no DOM, no I/O, so they can be unit tested.
import type { Discovered } from './types'

export interface DiscoverGroup {
  /** Parent directory, normalized to `/`; `''` keys items without a source. */
  key: string
  /** Short display label: the full directory, or drive + last two segments. */
  label: string
  /** Full source directory in Windows notation (empty for unknown sources). */
  hint: string
  items: Discovered[]
}

/** Language-neutral placeholder for items whose path has no parent directory. */
const UNKNOWN_LABEL = '?'

function normalizeSlashes(value: string): string {
  return value.replace(/\\/g, '/')
}

/** Parent directory of a discovered path with `/`, or `''` when there is none. */
export function sourceDirOf(path: string): string {
  const trimmed = normalizeSlashes(path).replace(/\/+$/, '')
  const cut = trimmed.lastIndexOf('/')
  return cut > 0 ? trimmed.slice(0, cut) : ''
}

/** `E:/a/b/c` -> `E:\…\b\c`, `E:/a/b` -> `E:\a\b`, `E:/a` -> `E:\a`. */
function groupLabel(dir: string): string {
  const parts = dir.split('/')
  if (parts.length <= 3) return parts.join('\\')
  const drive = /^[A-Za-z]:$/.test(parts[0]) ? `${parts[0]}\\…` : '…'
  return `${drive}\\${parts.slice(-2).join('\\')}`
}

function compareNames(a: Discovered, b: Discovered): number {
  const an = a.name.toLowerCase()
  const bn = b.name.toLowerCase()
  if (an !== bn) return an < bn ? -1 : 1
  if (a.path !== b.path) return a.path < b.path ? -1 : 1
  return 0
}

/** Known groups sort by directory; items without a source go last. */
function compareGroupKeys(a: string, b: string): number {
  if (a === b) return 0
  if (!a) return 1
  if (!b) return -1
  const an = a.toLowerCase()
  const bn = b.toLowerCase()
  if (an !== bn) return an < bn ? -1 : 1
  return a < b ? -1 : 1
}

export function groupDiscovered(items: Discovered[]): DiscoverGroup[] {
  const buckets = new Map<string, Discovered[]>()
  for (const item of items) {
    const key = sourceDirOf(item.path)
    const bucket = buckets.get(key)
    if (bucket) bucket.push(item)
    else buckets.set(key, [item])
  }

  const groups: DiscoverGroup[] = []
  for (const [key, bucket] of buckets) {
    groups.push({
      key,
      label: key ? groupLabel(key) : UNKNOWN_LABEL,
      hint: key ? key.replace(/\//g, '\\') : '',
      items: [...bucket].sort(compareNames)
    })
  }
  return groups.sort((a, b) => compareGroupKeys(a.key, b.key))
}
