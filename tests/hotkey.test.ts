import { describe, expect, it } from 'vitest'
import { acceleratorFromEvent, isModifierKey } from '../src/lib/hotkey'
import type { HotkeyEventLike } from '../src/lib/hotkey'

function ev(partial: Partial<HotkeyEventLike> = {}): HotkeyEventLike {
  return { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, key: '', ...partial }
}

describe('isModifierKey', () => {
  it('accepts the standalone modifier keys', () => {
    for (const key of ['Control', 'Shift', 'Alt', 'Meta']) {
      expect(isModifierKey(key)).toBe(true)
    }
  })

  it('rejects regular keys', () => {
    for (const key of ['k', 'K', '1', 'ArrowUp', 'F5', 'Escape', '']) {
      expect(isModifierKey(key)).toBe(false)
    }
  })
})

describe('acceleratorFromEvent', () => {
  it('reports modifier-only presses so recording can keep waiting', () => {
    for (const key of ['Control', 'Shift', 'Alt', 'Meta']) {
      expect(acceleratorFromEvent(ev({ key }))).toEqual({ accel: null, reason: 'modifier-only' })
    }
    // A modifier key is modifier-only even when another modifier is held.
    expect(acceleratorFromEvent(ev({ key: 'Control', shiftKey: true }))).toEqual({
      accel: null,
      reason: 'modifier-only'
    })
  })

  it('requires at least one modifier for a regular key', () => {
    for (const key of ['k', 'K', '1', 'ArrowUp', 'F5', 'Home', 'Insert', 'PageDown']) {
      expect(acceleratorFromEvent(ev({ key }))).toEqual({
        accel: null,
        reason: 'needs-modifier'
      })
    }
  })

  it('never turns reserved keys into hotkeys, even with modifiers', () => {
    for (const key of ['Escape', 'Tab', 'Enter', 'Backspace', ' ', 'Spacebar']) {
      expect(acceleratorFromEvent(ev({ key }))).toEqual({ accel: null, reason: 'reserved' })
    }
    expect(acceleratorFromEvent(ev({ key: 'Escape', ctrlKey: true }))).toEqual({
      accel: null,
      reason: 'reserved'
    })
    expect(acceleratorFromEvent(ev({ key: 'Tab', altKey: true }))).toEqual({
      accel: null,
      reason: 'reserved'
    })
  })

  it('formats a plain Ctrl+Alt+letter combination', () => {
    expect(acceleratorFromEvent(ev({ key: 'k', ctrlKey: true, altKey: true }))).toEqual({
      accel: 'Ctrl+Alt+K'
    })
    expect(acceleratorFromEvent(ev({ key: 'K', ctrlKey: true, altKey: true }))).toEqual({
      accel: 'Ctrl+Alt+K'
    })
  })

  it('emits modifiers in the fixed Ctrl+Alt+Shift+Super order', () => {
    expect(
      acceleratorFromEvent(
        ev({ key: 'k', ctrlKey: true, altKey: true, shiftKey: true, metaKey: true })
      )
    ).toEqual({ accel: 'Ctrl+Alt+Shift+Super+K' })
    // Pressing order does not matter: only the flags are read.
    expect(acceleratorFromEvent(ev({ key: 'ArrowUp', metaKey: true, ctrlKey: true }))).toEqual({
      accel: 'Ctrl+Super+Up'
    })
  })

  it('supports function keys, digits and navigation keys', () => {
    expect(acceleratorFromEvent(ev({ key: 'F5', altKey: true }))).toEqual({ accel: 'Alt+F5' })
    expect(acceleratorFromEvent(ev({ key: 'f24', ctrlKey: true }))).toEqual({
      accel: 'Ctrl+F24'
    })
    expect(acceleratorFromEvent(ev({ key: '1', ctrlKey: true }))).toEqual({ accel: 'Ctrl+1' })
    expect(acceleratorFromEvent(ev({ key: 'ArrowRight', ctrlKey: true }))).toEqual({
      accel: 'Ctrl+Right'
    })
    expect(acceleratorFromEvent(ev({ key: 'ArrowDown', shiftKey: true }))).toEqual({
      accel: 'Shift+Down'
    })
    expect(acceleratorFromEvent(ev({ key: 'PageUp', ctrlKey: true }))).toEqual({
      accel: 'Ctrl+PageUp'
    })
    expect(acceleratorFromEvent(ev({ key: 'PageDown', ctrlKey: true }))).toEqual({
      accel: 'Ctrl+PageDown'
    })
    expect(acceleratorFromEvent(ev({ key: 'Home', altKey: true }))).toEqual({ accel: 'Alt+Home' })
    expect(acceleratorFromEvent(ev({ key: 'End', altKey: true }))).toEqual({ accel: 'Alt+End' })
    expect(acceleratorFromEvent(ev({ key: 'Insert', ctrlKey: true }))).toEqual({
      accel: 'Ctrl+Insert'
    })
    expect(acceleratorFromEvent(ev({ key: 'Delete', ctrlKey: true }))).toEqual({
      accel: 'Ctrl+Delete'
    })
  })

  it('uppercases single letters but keeps other single keys as typed', () => {
    expect(acceleratorFromEvent(ev({ key: 'z', shiftKey: true }))).toEqual({ accel: 'Shift+Z' })
    expect(acceleratorFromEvent(ev({ key: '/', ctrlKey: true }))).toEqual({ accel: 'Ctrl+/' })
  })

  it('rejects unsupported named keys that have no accelerator form', () => {
    expect(acceleratorFromEvent(ev({ key: 'AudioVolumeUp', ctrlKey: true }))).toEqual({
      accel: null,
      reason: 'needs-modifier'
    })
    // F25 is outside the supported F1–F24 range.
    expect(acceleratorFromEvent(ev({ key: 'F25', ctrlKey: true }))).toEqual({
      accel: null,
      reason: 'needs-modifier'
    })
  })
})
