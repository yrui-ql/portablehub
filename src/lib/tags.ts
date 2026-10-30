/** User-defined category tags: pure helpers shared by the UI and its tests. */

export const MAX_TAGS = 20
export const MAX_TAG_LENGTH = 24

export interface TagCount {
  tag: string
  count: number
}

/**
 * Trim, drop empty/over-long entries, de-duplicate case-insensitively and cap
 * the list. Mirrors the Rust `normalize_tags` helper.
 */
export function normalizeTags(tags: string[]): string[] {
  const out: string[] = []
  for (const raw of tags) {
    const tag = raw.trim()
    if (!tag || tag.length > MAX_TAG_LENGTH) continue
    if (out.some((existing) => existing.toLowerCase() === tag.toLowerCase())) continue
    out.push(tag)
    if (out.length >= MAX_TAGS) break
  }
  return out
}

/** Every tag in use, most-used first, ties broken by locale-aware order. */
export function collectTags(apps: { tags?: string[] }[]): TagCount[] {
  const counts = new Map<string, TagCount>()
  for (const app of apps) {
    for (const tag of app.tags ?? []) {
      const key = tag.toLowerCase()
      const entry = counts.get(key)
      if (entry) entry.count++
      else counts.set(key, { tag, count: 1 })
    }
  }
  return [...counts.values()].sort(
    (a, b) => b.count - a.count || a.tag.localeCompare(b.tag, 'zh-CN')
  )
}

/** True when the app carries every active tag (AND semantics). */
export function matchesTags(app: { tags?: string[] }, activeTags: string[]): boolean {
  return activeTags.every((tag) => (app.tags ?? []).includes(tag))
}
