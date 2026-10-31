import type { Discovered } from './types'

/**
 * Merge same-name scan/discover hits into one row.
 *
 * A portable library often contains the same app in several folders (old
 * version, backup, duplicate extraction). Showing every path as its own badge
 * makes the review screen unreadable and invites double registration. Keep the
 * shortest path as the representative and expose only a count; the user can
 * still inspect the hidden locations via the row title.
 */
export function dedupeDiscovered(items: Discovered[]): Discovered[] {
  const byName = new Map<string, Discovered>()

  for (const item of items) {
    const name = item.name.trim()
    const key = (name || item.path).toLocaleLowerCase()
    const previous = byName.get(key)
    if (!previous) {
      byName.set(key, { ...item, name })
      continue
    }

    const count = (previous.duplicateCount ?? 0) + 1
    // Prefer the shortest path: it is usually the canonical install /
    // top-level folder, not a backup nested deep in another directory.
    const representative =
      item.path.length < previous.path.length
        ? { ...item, name, duplicateCount: count }
        : { ...previous, duplicateCount: count }
    byName.set(key, representative)
  }

  return [...byName.values()].map(({ duplicateCount, ...item }) =>
    duplicateCount ? { ...item, duplicateCount } : item
  )
}
