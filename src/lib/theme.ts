import type { Theme } from './types'

/**
 * Apply a theme by writing `--ph-*` custom properties on <html> and injecting the
 * optional CSS block as text (never HTML).
 */
export function applyTheme(theme: Theme, doc: Document = document): void {
  const root = doc.documentElement

  for (const name of Array.from(root.style)) {
    if (name.startsWith('--ph-')) root.style.removeProperty(name)
  }
  for (const [key, value] of Object.entries(theme.vars ?? {})) {
    if (key.startsWith('--ph-')) root.style.setProperty(key, value)
  }

  let el = doc.getElementById('ph-theme-css') as HTMLStyleElement | null
  if (!el) {
    el = doc.createElement('style')
    el.id = 'ph-theme-css'
    doc.head.appendChild(el)
  }
  el.textContent = theme.css ?? ''
}

/** Toggle pixelated rendering for app icons; independent of the active theme. */
export function applyPixelIcons(enabled: boolean, doc: Document = document): void {
  doc.documentElement.dataset.pixelIcons = enabled ? 'true' : 'false'
}

/** Use the bundled pixel font for the whole UI (user preference). */
export function applyPixelFont(enabled: boolean, doc: Document = document): void {
  doc.documentElement.dataset.pixelFont = enabled ? 'true' : 'false'
}

export function fallbackLetter(name: string): string {
  const trimmed = name.trim()
  return trimmed ? trimmed.charAt(0).toUpperCase() : '?'
}

export function hashColor(name: string): string {
  let h = 0
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0
  const r = 64 + (h & 0x7f)
  const g = 64 + ((h >> 8) & 0x7f)
  const b = 64 + ((h >> 16) & 0x7f)
  return `rgb(${r}, ${g}, ${b})`
}
