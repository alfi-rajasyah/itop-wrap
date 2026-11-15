import { useEffect } from 'react'
import { useSystemTheme } from '@/hooks/useSystemTheme'
import { usePreventReload } from '@/lib/keys'
import MenuBarPanelScreen from '@/screens/MenuBarPanelScreen'

/** Root of the `menubar` window: a borderless, transparent window holding the rounded panel. */
export default function MenuBarApp() {
  useSystemTheme()
  usePreventReload()
  useEffect(() => {
    // The window itself is transparent; only the panel paints.
    const targets = [document.documentElement, document.body]
    targets.forEach((el) => el.style.setProperty('background', 'transparent'))
    return () => targets.forEach((el) => el.style.removeProperty('background'))
  }, [])
  return <MenuBarPanelScreen />
}
