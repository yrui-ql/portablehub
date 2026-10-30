/**
 * The semantic categories are a fixed vocabulary (shared with the Rust
 * classifier). Users can assign or clear one manually; AI only proposes.
 */
export const CATEGORY_ORDER = [
  'system',
  'network',
  'dev',
  'game',
  'media',
  'document',
  'security',
  'storage',
  'comm',
  'runtime',
  'other'
] as const

export type Category = (typeof CATEGORY_ORDER)[number]

export function isCategory(value: string): value is Category {
  return (CATEGORY_ORDER as readonly string[]).includes(value)
}
