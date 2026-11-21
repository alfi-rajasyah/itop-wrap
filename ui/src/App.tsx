import { useEffect, useRef } from 'react'
import { isTauri, useModel } from '@/backend'
import { useSystemTheme } from '@/hooks/useSystemTheme'
import { usePreventReload } from '@/lib/keys'
import LoginScreen from '@/screens/LoginScreen'
import NewTicketForm from '@/screens/NewTicketForm'
import ShellScreen from '@/screens/ShellScreen'
import SettingsScreen from '@/screens/SettingsScreen'
import { ResolveWindowForm } from '@/screens/TicketSheets'

type WindowKind = 'main' | 'new-ticket' | 'resolve' | 'settings'

/**
 * The `new-ticket` window loads index.html?window=new-ticket (contract §6, Windows), the menu
 * bar's Resolve window index.html?window=resolve, Settings index.html?window=settings.
 */
function windowKind(): WindowKind {
  if (typeof window === 'undefined') return 'main'
  const kind = new URLSearchParams(window.location.search).get('window')
  return kind === 'new-ticket' || kind === 'resolve' || kind === 'settings' ? kind : 'main'
}

/** Root of the main window (W/App.swift RootView), the New Ticket, Resolve and Settings windows. */
export default function App({ kind = windowKind() }: { kind?: WindowKind }) {
  useSystemTheme()
  usePreventReload()
  if (kind === 'new-ticket') return <NewTicketWindow />
  if (kind === 'resolve') return <ResolveWindow />
  if (kind === 'settings') return <SettingsWindow />
  return <MainWindow />
}

function MainWindow() {
  const [session] = useModel('session')
  if (!session) return <div className="h-screen bg-app" />
  return session.type === 'signedIn' ? <ShellScreen /> : <LoginScreen />
}

/**
 * Like `.windowResizability(.contentSize)`: the window's height follows the content at a fixed
 * width. Returns the ref for the content's root.
 */
function useFitWindowHeight(width: number) {
  const root = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const el = root.current
    if (!el || typeof ResizeObserver === 'undefined' || !isTauri()) return
    let cancelled = false
    // What we last asked for. The window came out a title bar short of it (the form's buttons
    // were cut off), so instead of trusting setSize we check the real viewport after each
    // resize and ask again for the difference; once it fits, nothing more is sent.
    let requested = 0
    const fit = () => {
      const height = Math.ceil(el.getBoundingClientRect().height)
      if (height <= 0 || cancelled) return
      const short = height - window.innerHeight
      const next = requested === 0 ? height : requested + short
      if (short === 0 || next === requested) return
      requested = next
      void import('@tauri-apps/api/window').then(({ getCurrentWindow, LogicalSize }) => {
        if (!cancelled) void getCurrentWindow().setSize(new LogicalSize(width, next))
      })
    }
    const observer = new ResizeObserver(fit)
    observer.observe(el)
    window.addEventListener('resize', fit)
    return () => {
      cancelled = true
      observer.disconnect()
      window.removeEventListener('resize', fit)
    }
  }, [width])
  return root
}

/** New Ticket is 480 pt wide (W/NewTicketView.swift:141). */
const NEW_TICKET_WIDTH = 480

/**
 * The New Ticket window (NEW-2): it has the system title bar, so no drag strip of its own, and
 * like `.windowResizability(.contentSize)` its height follows the form at 480 pt wide.
 */
function NewTicketWindow() {
  const root = useFitWindowHeight(NEW_TICKET_WIDTH)
  return (
    <div ref={root} className="bg-surface text-ink" style={{ width: NEW_TICKET_WIDTH }}>
      <NewTicketForm instance="window" />
    </div>
  )
}

/** The Resolve window is as wide as the Resolve sheet. */
const RESOLVE_WIDTH = 500

/** The menu bar's Resolve window: the Resolve form alone, its height following the form. */
function ResolveWindow() {
  const root = useFitWindowHeight(RESOLVE_WIDTH)
  return (
    <div ref={root} className="bg-surface text-ink" style={{ width: RESOLVE_WIDTH }}>
      <ResolveWindowForm />
    </div>
  )
}

/** Settings is 440 pt wide; its height follows the content. */
const SETTINGS_WIDTH = 440

function SettingsWindow() {
  const root = useFitWindowHeight(SETTINGS_WIDTH)
  return (
    <div ref={root} className="bg-surface text-ink" style={{ width: SETTINGS_WIDTH }}>
      <SettingsScreen />
    </div>
  )
}
