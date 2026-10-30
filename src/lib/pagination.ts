/**
 * REQ-0042 · render windowing for large libraries.
 *
 * The main grid only materializes a bounded number of `AppCard` components at a
 * time: with ~10k apps, rendering every card at once would create 10k DOM
 * subtrees and block the UI. Pure helpers (no DOM, no state) so the behaviour is
 * unit-testable.
 */

/** Cards rendered for one grid / one group section before "show more". */
export const PAGE_SIZE = 200

/**
 * First `limit` items. A non-positive or non-finite limit yields an empty list,
 * a limit larger than the list simply returns everything.
 */
export function pageSlice<T>(list: readonly T[], limit: number): T[] {
  if (!Number.isFinite(limit) || limit <= 0) return []
  return list.slice(0, Math.trunc(limit))
}

/**
 * The next render limit after "show more": one more page, never shrinking.
 * Non-positive input is clamped so a stale 0/negative limit still advances.
 */
export function nextPage(limit: number): number {
  const current = Number.isFinite(limit) && limit > 0 ? Math.trunc(limit) : 0
  return current + PAGE_SIZE
}
