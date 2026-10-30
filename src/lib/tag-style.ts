import type { TagStyle } from './types'

/** Preset tag colours (plus empty = theme default). */
export const TAG_COLORS = [
  '',
  '#4ade80', // green
  '#22d3ee', // cyan
  '#fbbf24', // amber
  '#f87171', // red
  '#a78bfa', // violet
  '#f472b6', // pink
  '#60a5fa', // blue
  '#9ca3af' // grey
]

export function styleOf(
  styles: Record<string, TagStyle> | undefined,
  tag: string
): TagStyle | undefined {
  return styles?.[tag]
}

export function tagCss(style: TagStyle | undefined): string {
  if (!style) return ''
  const parts: string[] = []
  if (style.color) {
    parts.push(`color:${style.color}`, `border-color:${style.color}`)
  }
  return parts.join(';')
}

export function tagText(style: TagStyle | undefined, tag: string): string {
  return style?.icon ? `${style.icon} ${tag}` : tag
}
