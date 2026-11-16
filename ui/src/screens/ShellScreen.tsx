import { useState, type ReactNode } from 'react'
import { isTauri, useModel, type ShellSnapshot, type SidebarItem } from '@/backend'
import {
  AppShell,
  BuildingIcon,
  Dialog,
  IconButton,
  PeopleIcon,
  ServerIcon,
  SignOutIcon,
  SidebarIcon,
  Sidebar,
  TeamIcon,
  TrayIcon,
  type SidebarSection,
} from '@/components/kit'
import { isCommand, useWindowKeyDown } from '@/lib/keys'
import CmdbScreen from './CmdbScreen'
import NewTicketForm from './NewTicketForm'
import OrgsScreen from './OrgsScreen'
import PeopleScreen from './PeopleScreen'
import TicketListScreen from './TicketListScreen'

/*
 * The signed-in window (W/ShellView.swift): the sidebar with its two sections, counts, ⌘1…5
 * hints and the account menu, then the selected section. New ticket opens as a sheet over it
 * (SHELL-14). Sign out asks first (AUTH-29).
 */

const ICONS: Record<SidebarItem, ReactNode> = {
  myTickets: <TrayIcon />,
  teamTickets: <TeamIcon />,
  configItems: <ServerIcon />,
  organizations: <BuildingIcon />,
  people: <PeopleIcon />,
}

/** SHELL-3: two fixed sections. */
const SECTIONS: { id: string; title: string; items: SidebarItem[] }[] = [
  { id: 'tickets', title: 'Tickets', items: ['myTickets', 'teamTickets'] },
  { id: 'cmdb', title: 'CMDB', items: ['configItems', 'organizations', 'people'] },
]

/** KEY-2: the View menu's shortcuts, in sidebar order. */
const SHORTCUTS: Record<SidebarItem, string> = {
  myTickets: '1',
  teamTickets: '2',
  configItems: '3',
  organizations: '4',
  people: '5',
}

export default function ShellScreen() {
  const [shell, send] = useModel('shell')
  const [, sendSession] = useModel('session')
  const [confirmSignOut, setConfirmSignOut] = useState(false)

  // ⌘1…5 and ⌘N are native menu items in the app (KEY-1, KEY-2). The browser mock has no
  // menu bar, so the same shortcuts are handled here only outside Tauri.
  useWindowKeyDown((e) => {
    if (isTauri() || !shell) return
    const item = (Object.keys(SHORTCUTS) as SidebarItem[]).find((k) => isCommand(e, SHORTCUTS[k]))
    if (item) {
      e.preventDefault()
      void send({ type: 'select', item })
    } else if (isCommand(e, 'n')) {
      e.preventDefault()
      void send({ type: 'newTicket' })
    } else if (e.key.toLowerCase() === 's' && e.ctrlKey && e.metaKey && !e.altKey && !e.shiftKey) {
      // View ▸ Hide Sidebar / Show Sidebar (⌃⌘S) is a native menu item in the app.
      e.preventDefault()
      void send({ type: 'setSidebarVisible', visible: !shell.sidebarVisible })
    }
  })

  if (!shell) return <div className="h-screen bg-app" />

  const toggle = (
    <IconButton
      label={shell.sidebarVisible ? 'Hide Sidebar' : 'Show Sidebar'}
      tooltip={shell.sidebarVisible ? 'Hide Sidebar' : 'Show Sidebar'}
      icon={<SidebarIcon />}
      onClick={() => void send({ type: 'setSidebarVisible', visible: !shell.sidebarVisible })}
    />
  )

  return (
    <div className="h-screen">
      <AppShell
        sidebarVisible={shell.sidebarVisible}
        sidebarToggle={toggle}
        sidebar={
          <Sidebar
            sections={sidebarSections(shell)}
            selectedId={shell.selection}
            onSelect={(id) => void send({ type: 'select', item: id as SidebarItem })}
            header={toggle}
            width={shell.sidebarWidth}
            onWidthChange={(width) => void send({ type: 'setSidebarWidth', width })}
            user={{
              name: shell.account.displayName,
              detail: shell.account.host,
              initials: shell.account.initials,
              ariaLabel: `Account: signed in as ${shell.account.displayName} on ${shell.account.host}`,
              tooltip: `${shell.account.displayName} · ${shell.account.host}`,
              menu: [
                { id: 'who', label: `${shell.account.displayName} · ${shell.account.host}`, disabled: true },
                { kind: 'separator', id: 'sep' },
                { id: 'signOut', label: 'Sign out…', icon: <SignOutIcon />, onSelect: () => setConfirmSignOut(true) },
              ],
            }}
          />
        }
      >
        <Section selection={shell.selection} />
      </AppShell>

      <NewTicketForm instance="sheet" />

      <Dialog
        open={confirmSignOut}
        role="alertdialog"
        title={shell.signOutConfirm.title}
        subtitle={shell.signOutConfirm.message}
        confirmLabel="Sign out"
        cancelLabel="Cancel"
        width={380}
        onCancel={() => setConfirmSignOut(false)}
        onConfirm={() => {
          setConfirmSignOut(false)
          void sendSession({ type: 'signOut' })
        }}
      />
    </div>
  )
}

function Section({ selection }: { selection: SidebarItem }) {
  switch (selection) {
    case 'myTickets':
      return <TicketListScreen key="mine" scope="mine" />
    case 'teamTickets':
      return <TicketListScreen key="team" scope="team" />
    case 'configItems':
      return <CmdbScreen />
    case 'organizations':
      return <OrgsScreen />
    case 'people':
      return <PeopleScreen />
  }
}

function sidebarSections(shell: ShellSnapshot): SidebarSection[] {
  return SECTIONS.map((section) => ({
    id: section.id,
    title: section.title,
    items: section.items.flatMap((item) => {
      const row = shell.sidebar.find((r) => r.item === item)
      if (!row) return []
      return [
        {
          id: item,
          label: row.title,
          icon: ICONS[item],
          shortcut: `⌘${SHORTCUTS[item]}`,
          // SHELL-4: Rust sends a count only for My tickets, only when > 0.
          ...(row.count != null ? { count: row.count, countTone: 'need' as const, countLabel: `${row.count} need you` } : {}),
        },
      ]
    }),
  }))
}
