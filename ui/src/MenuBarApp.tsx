import { useEffect, useRef } from 'react'
import { useSystemTheme } from '@/hooks/useSystemTheme'
import { usePreventReload } from '@/lib/keys'
import MenuBarPanelScreen from '@/screens/MenuBarPanelScreen'

const PANEL_WIDTH = 360

/** Root of the `menubar` window: a borderless, transparent window holding the rounded panel. */
export default function MenuBarApp() {
  useSystemTheme()
  usePreventReload()
  const root = useRef<HTMLDivElement>(null)
  useEffect(() => {
    // The window itself is transparent; only the panel paints.
    const targets = [document.documentElement, document.body]
    targets.forEach((el) => el.style.setProperty('background', 'transparent'))
    return () => targets.forEach((el) => el.style.removeProperty('background'))
  }, [])
  useEffect(() => {
    // The panel's height follows its content (MB-15); the window keeps its top edge.
    const el = root.current
    if (!el || typeof ResizeObserver === 'undefined' || !('__TAURI_INTERNALS__' in window)) return
    let cancelled = false
    const observer = new ResizeObserver(() => {
      const height = Math.ceil(el.getBoundingClientRect().height)
      if (height <= 0) return
      void import('@tauri-apps/api/window').then(({ getCurrentWindow, LogicalSize }) => {
        if (!cancelled) void getCurrentWindow().setSize(new LogicalSize(PANEL_WIDTH, height))
      })
    })
    observer.observe(el)
    return () => {
      cancelled = true
      observer.disconnect()
    }
  }, [])
  return (
    <div ref={root}>
      <MenuBarPanelScreen />
    </div>
  )
}
