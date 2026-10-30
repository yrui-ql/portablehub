import { initialsOf } from './pinyin'

export interface Filterable {
  name: string
  tags?: string[]
  autostart?: boolean
}

export type AutostartFilter = 'off' | 'include' | 'exclude'

/**
 * Case-insensitive substring match over the app name, pinyin initials and tags.
 * Typing `wx` therefore finds a name whose characters start with w- and x-
 * initials, while an English or Chinese substring keeps matching the plain name.
 *
 * Included tags are AND-combined (an app must carry every one); excluded tags
 * remove any app carrying at least one of them. This lets a group express
 * "only tagged X" and "hide tagged Y" at the same time.
 * The virtual autostart tag filters on the `autostart` flag.
 * Empty/whitespace query and empty filters return the original list.
 */
export function filterApps<T extends Filterable>(
  apps: T[],
  query: string,
  activeTags: string[] = [],
  excludedTags: string[] = [],
  autostart: AutostartFilter = 'off'
): T[] {
  const q = query.trim().toLowerCase()
  if (!q && activeTags.length === 0 && excludedTags.length === 0 && autostart === 'off') {
    return apps
  }
  return apps.filter((a) => {
    const tags = a.tags ?? []
    if (autostart === 'include' && a.autostart !== true) return false
    if (autostart === 'exclude' && a.autostart === true) return false
    if (activeTags.length && !activeTags.every((tag) => tags.includes(tag))) {
      return false
    }
    if (excludedTags.length && excludedTags.some((tag) => tags.includes(tag))) {
      return false
    }
    if (!q) return true
    return (
      a.name.toLowerCase().includes(q) ||
      initialsOf(a.name).includes(q) ||
      tags.some((tag) => tag.toLowerCase().includes(q))
    )
  })
}
