// BUG-0013 · keyboard-event → Tauri accelerator formatting for the hotkey
// recorder. Pure module: no DOM, no i18n, no Tauri imports so it can be unit
// tested and reused by any component.
//
// Accelerators are emitted in a fixed modifier order — Ctrl+Alt+Shift+Super —
// independent of the order the keys were pressed.

export type AcceleratorFailureReason = 'modifier-only' | 'needs-modifier' | 'reserved'

export type AcceleratorResult =
  | { accel: string }
  | { accel: null; reason: AcceleratorFailureReason }

/** The subset of `KeyboardEvent` the formatter needs. */
export interface HotkeyEventLike {
  ctrlKey: boolean
  shiftKey: boolean
  altKey: boolean
  metaKey: boolean
  key: string
}

const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'OS'])

// Keys that own the interaction: pressing them must never become a hotkey.
const RESERVED_KEYS = new Set(['Escape', 'Tab', 'Enter', 'Backspace', 'Space'])

// Named non-printable keys that map onto Tauri accelerator names.
const NAMED_KEYS: Record<string, string> = {
  arrowup: 'Up',
  arrowdown: 'Down',
  arrowleft: 'Left',
  arrowright: 'Right',
  up: 'Up',
  down: 'Down',
  left: 'Left',
  right: 'Right',
  home: 'Home',
  end: 'End',
  insert: 'Insert',
  delete: 'Delete',
  pageup: 'PageUp',
  pagedown: 'PageDown'
}

/** True for the standalone modifier keys (Control/Shift/Alt/Meta). */
export function isModifierKey(key: string): boolean {
  return MODIFIER_KEYS.has(key)
}

/** Legacy/alias key names normalized to the names used in `RESERVED_KEYS`. */
function normalizeReservedKey(key: string): string {
  if (key === ' ' || key === 'Spacebar') return 'Space'
  if (key === 'Esc') return 'Escape'
  return key
}

/** Format the non-modifier key as a Tauri accelerator key name, if possible. */
function mainKeyName(key: string): string | null {
  if (key.length === 1) {
    // Letters are canonically uppercase; digits and punctuation pass through.
    return /[a-z]/i.test(key) ? key.toUpperCase() : key
  }
  const named = NAMED_KEYS[key.toLowerCase()]
  if (named) return named
  if (/^F([1-9]|1[0-9]|2[0-4])$/i.test(key)) return key.toUpperCase()
  return null
}

/**
 * Turn a keydown event into an accelerator string.
 * - modifier-only presses report `modifier-only` (keep listening);
 * - keys without any modifier report `needs-modifier`;
 * - reserved keys (Esc, Tab, Enter, Backspace, Space) report `reserved`;
 * - everything else becomes `Ctrl+Alt+Shift+Super+Key` in that fixed order.
 */
export function acceleratorFromEvent(e: HotkeyEventLike): AcceleratorResult {
  if (isModifierKey(e.key)) return { accel: null, reason: 'modifier-only' }
  if (RESERVED_KEYS.has(normalizeReservedKey(e.key))) {
    return { accel: null, reason: 'reserved' }
  }

  const hasModifier = e.ctrlKey || e.altKey || e.shiftKey || e.metaKey
  if (!hasModifier) return { accel: null, reason: 'needs-modifier' }

  const main = mainKeyName(e.key)
  // Unknown named keys (dead keys, media keys, …) cannot be formatted reliably.
  if (!main) return { accel: null, reason: 'needs-modifier' }

  const parts: string[] = []
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.altKey) parts.push('Alt')
  if (e.shiftKey) parts.push('Shift')
  if (e.metaKey) parts.push('Super')
  parts.push(main)
  return { accel: parts.join('+') }
}
