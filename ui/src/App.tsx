import { useModel } from '@/backend'
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

/** Bare chrome: a draggable strip clear of the traffic lights, then the form. */
function NewTicketWindow() {
  return (
    <div className="flex h-screen flex-col bg-surface text-ink">
      <div data-tauri-drag-region className="h-7 shrink-0" />
      <div className="min-h-0 flex-1 overflow-y-auto">
        <NewTicketForm instance="window" />
      </div>
    </div>
  )
}
