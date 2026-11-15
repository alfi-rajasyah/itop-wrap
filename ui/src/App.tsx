import { useEffect, useRef } from 'react'
import { isTauri, useModel } from '@/backend'
import { useSystemTheme } from '@/hooks/useSystemTheme'
import { usePreventReload } from '@/lib/keys'
import LoginScreen from '@/screens/LoginScreen'
import NewTicketForm from '@/screens/NewTicketForm'
import ShellScreen from '@/screens/ShellScreen'

/** The `new-ticket` window loads index.html?window=new-ticket (contract §6, Windows). */
function windowKind(): 'main' | 'new-ticket' {
  if (typeof window === 'undefined') return 'main'
  return new URLSearchParams(window.location.search).get('window') === 'new-ticket' ? 'new-ticket' : 'main'
}

/** Root of the main window (W/App.swift RootView) and of the New Ticket window. */
export default function App({ kind = windowKind() }: { kind?: 'main' | 'new-ticket' }) {
  useSystemTheme()
  usePreventReload()
  return kind === 'new-ticket' ? <NewTicketWindow /> : <MainWindow />
}

function MainWindow() {
  const [session] = useModel('session')
  if (!session) return <div className="h-screen bg-app" />
  return session.type === 'signedIn' ? <ShellScreen /> : <LoginScreen />
}

/** New Ticket is 480 pt wide (W/NewTicketView.swift:141). */
const NEW_TICKET_WIDTH = 480

/**
 * The New Ticket window (NEW-2): it has the system title bar, so no drag strip of its own, and
 * like `.windowResizability(.contentSize)` its height follows the form at 480 pt wide.
 */
function NewTicketWindow() {
  const root = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const el = root.current
    if (!el || typeof ResizeObserver === 'undefined' || !isTauri()) return
    let cancelled = false
    const observer = new ResizeObserver(() => {
      const height = Math.ceil(el.getBoundingClientRect().height)
      if (height <= 0) return
      void import('@tauri-apps/api/window').then(({ getCurrentWindow, LogicalSize }) => {
        if (!cancelled) void getCurrentWindow().setSize(new LogicalSize(NEW_TICKET_WIDTH, height))
      })
    })
    observer.observe(el)
    return () => {
      cancelled = true
      observer.disconnect()
    }
  }, [])
  return (
    <div ref={root} className="bg-surface text-ink" style={{ width: NEW_TICKET_WIDTH }}>
      <NewTicketForm instance="window" />
    </div>
  )
}
