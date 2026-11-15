import { useEffect, useRef, type KeyboardEvent as ReactKeyboardEvent } from 'react'

/** ⌘ on macOS (Ctrl elsewhere, for the browser mock), with no other modifier. */
export function isCommand(e: KeyboardEvent | ReactKeyboardEvent, key: string): boolean {
  return (e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === key
}

/**
 * Listens for keydown on the window while mounted. The handler is kept in a ref, so it always
 * sees the latest props without re-subscribing.
 */
export function useWindowKeyDown(handler: (e: KeyboardEvent) => void, enabled = true) {
  const ref = useRef(handler)
  ref.current = handler
  useEffect(() => {
    if (!enabled) return
    const listener = (e: KeyboardEvent) => ref.current(e)
    window.addEventListener('keydown', listener)
    return () => window.removeEventListener('keydown', listener)
  }, [enabled])
}

/**
 * ⌘R must never reload the web view (contract §6). Screens that refresh something handle it
 * themselves; this only stops the reload everywhere else in the window.
 */
/** A sheet, dialog or the image viewer is up (they all mark themselves `aria-modal`). */
export function isModalOpen(): boolean {
  return document.querySelector('[aria-modal="true"]') !== null
}

export function usePreventReload() {
  useWindowKeyDown((e) => {
    if (isCommand(e, 'r')) e.preventDefault()
  })
}
