// REQ-0022 · deterministic app grouping by source folder.
//
// Rules (see backlog/REQ-0022.md):
// - Relative paths (the hub's own `Apps\…` tree, or anything without a drive)
//   fall into the `local` group; its `label` stays empty so the UI can render
//   the localized `app.groupLocal` text — this module must not depend on i18n.
// - Absolute paths are grouped by drive + the first two folders of the exe's
//   parent directory, e.g. `E:\Work\Tools\SomeApp\SomeApp.exe` → group
//   `E:\Work\Tools`. The label drops the drive (`Work\Tools`); the hint keeps
//   the full group directory as a tooltip. Separators are normalized to `\`.
// - The local group is first, the rest sort by label; apps inside a group sort
//   by name, case-insensitively.
// Pure module: no DOM, no translations, no I/O.
import type { AppView } from './types'

/** Group key for apps that live inside the hub folder (relative paths). */
export const LOCAL_GROUP_KEY = 'local'

export interface AppGroup {
  key: string
  label: string
  hint: string
  apps: AppView[]
}

interface GroupSource {
  key: string
  label: string
  hint: string
}

const LOCAL_SOURCE: GroupSource = { key: LOCAL_GROUP_KEY, label: '', hint: '' }

/** Split a path remainder into non-empty segments; trailing separators ignored. */
function splitSegments(rest: string): string[] {
  return rest
    .replace(/\\+$/, '')
    .split('\\')
    .filter((part) => part.length > 0)
}

/** Build the group coordinates for a drive path remainder after `X:`. */
function sourceForDrive(drive: string, rest: string): GroupSource {
  const segments = splitSegments(rest)
  // The trailing segment is the executable file; grouping is about its folder.
  const folders = segments.slice(0, -1)
  const groupFolders = folders.slice(0, 2)
  const key = groupFolders.length
    ? `${drive}\\${groupFolders.join('\\')}`
    : `${drive}\\`
  // A drive-root group still needs a visible label; only `local` uses ''.
  const label = groupFolders.length ? groupFolders.join('\\') : drive
  return { key, label, hint: key }
}

function sourceForUnc(rest: string): GroupSource {
  const segments = splitSegments(rest)
  const folders = segments.slice(0, -1)
  const groupFolders = folders.slice(0, 2)
  const key = `\\\\${groupFolders.join('\\')}`
  return { key, label: groupFolders.join('\\'), hint: key }
}

/**
 * Resolve the group coordinates of one app path.
 * The returned `label` is empty only for the `local` group.
 */
export function groupKeyOf(path: string): GroupSource {
  const raw = typeof path === 'string' ? path.trim() : ''
  if (!raw) return LOCAL_SOURCE
  const normalized = raw.replace(/\//g, '\\')

  const drive = /^([A-Za-z]):(.*)$/.exec(normalized)
  if (drive) return sourceForDrive(`${drive[1].toUpperCase()}:`, drive[2])

  // UNC path: `\\server\share\folder\app.exe` groups by server + share.
  if (normalized.startsWith('\\\\')) return sourceForUnc(normalized.slice(2))

  // Anything else (relative, rooted without a drive) belongs to the hub folder.
  return LOCAL_SOURCE
}

function compareNames(a: string, b: string): number {
  const lowerA = a.toLowerCase()
  const lowerB = b.toLowerCase()
  if (lowerA < lowerB) return -1
  if (lowerA > lowerB) return 1
  if (a < b) return -1
  if (a > b) return 1
  return 0
}

function compareGroupKeys(a: AppGroup, b: AppGroup): number {
  const aLocal = a.key === LOCAL_GROUP_KEY
  const bLocal = b.key === LOCAL_GROUP_KEY
  if (aLocal || bLocal) return aLocal === bLocal ? 0 : aLocal ? -1 : 1
  return compareNames(a.label, b.label) || compareNames(a.key, b.key)
}

/**
 * Group apps by source folder. Returns a fresh array (and fresh group app
 * arrays), sorted as described above; the input array is left untouched.
 */
export function groupApps(apps: AppView[]): AppGroup[] {
  if (!apps.length) return []

  const groups = new Map<string, AppGroup>()
  for (const app of apps) {
    const source = groupKeyOf(app.path)
    // Windows paths are case-insensitive: merge differing casing into the first
    // seen key rather than emitting two groups for the same folder.
    const mapKey = source.key === LOCAL_GROUP_KEY ? LOCAL_GROUP_KEY : source.key.toLowerCase()
    let group = groups.get(mapKey)
    if (!group) {
      group = { key: source.key, label: source.label, hint: source.hint, apps: [] }
      groups.set(mapKey, group)
    }
    group.apps.push(app)
  }

  const result = [...groups.values()]
  for (const group of result) group.apps.sort((a, b) => compareNames(a.name, b.name))
  result.sort(compareGroupKeys)
  return result
}
