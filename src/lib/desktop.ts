import { listen, type UnlistenFn } from '@tauri-apps/api/event'

/**
 * Thin wrapper around Tauri window / dialog APIs.
 *
 * The UI also runs in a plain browser (Playwright E2E via `VITE_E2E=1`, and
 * `vite preview`), where `__TAURI_INTERNALS__` is absent and any direct call
 * into `@tauri-apps/api` throws. Outside Tauri every function here is inert.
 *
 * Desktop failures are logged instead of thrown: callers are UI event handlers
 * (title bar buttons), where an unhandled rejection would be silent anyway.
 * A missing internals object outside E2E means the desktop detection itself is
 * broken, so it is surfaced once on the console.
 */
export const isDesktop: boolean =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

const isE2E = import.meta.env.VITE_E2E === '1'

if (!isDesktop && !isE2E) {
  console.warn('[desktop] Tauri internals not found: window/dialog actions are inert')
}

async function currentWindow() {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  return getCurrentWindow()
}

async function desktopAction(action: () => Promise<void>): Promise<void> {
  if (!isDesktop) return
  try {
    await action()
  } catch (error) {
    console.error('[desktop] action failed:', error)
  }
}

export function minimizeWindow(): Promise<void> {
  return desktopAction(async () => (await currentWindow()).minimize())
}

export function toggleMaximizeWindow(): Promise<void> {
  return desktopAction(async () => (await currentWindow()).toggleMaximize())
}

export function closeWindow(): Promise<void> {
  return desktopAction(async () => (await currentWindow()).close())
}

/** Subscribe to a Tauri event; resolves to a no-op unlisten outside Tauri. */
export async function listenSafe<T>(
  event: string,
  handler: (payload: T) => void
): Promise<UnlistenFn> {
  if (!isDesktop) return () => {}
  try {
    return await listen<T>(event, (e) => handler(e.payload as T))
  } catch (error) {
    console.error(`[desktop] listen failed for ${event}:`, error)
    return () => {}
  }
}

/**
 * Native directory picker. Returns the chosen folder; `null` means either
 * "cancelled" or "not running under Tauri" — callers treat both as a no-op.
 */
export async function pickDirectory(defaultPath?: string): Promise<string | null> {
  if (!isDesktop) return null
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const picked = await open({ directory: true, multiple: false, defaultPath })
    return typeof picked === 'string' ? picked : null
  } catch (error) {
    console.error('[desktop] directory picker failed:', error)
    return null
  }
}
