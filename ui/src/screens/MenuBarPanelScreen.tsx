import { isTauri, useModel, type MenuRow, type Send } from '@/backend'
import {
  Button,
  LoadingState,
  MenuBarFooter,
  MenuBarPanel,
  MenuBarSection,
  MenuBarTicketRow,
  MenuBarTopRow,
} from '@/components/kit'
import { isCommand, useWindowKeyDown } from '@/lib/keys'
import { useModelText } from '@/lib/useModelText'
import { TimerNotice } from './list/TimerNotice'

/*
 * The menu bar panel (W/MenuBarView.swift MenuBarPanel, MB-7…16): search and New ticket, the
 * timer notice, My tickets that want attention grouped by status with the running one on top,
 * and the footer with Refresh, "Updated …" and Open iTop Wrap. 360 pt wide, in a borderless
 * transparent window, so the panel draws its own rounded edge.
 */
export default function MenuBarPanelScreen() {
  const [menu, send] = useModel('menubar')
  const [timer, sendTimer] = useModel('timer')

  useWindowKeyDown((e) => {
    if (!menu) return
    if (isCommand(e, 'r')) {
      // KEY-3, KEY-8: React handles ⌘R here and stops the reload.
      e.preventDefault()
      if (!menu.refreshing) void send({ type: 'refresh' })
    } else if (!isTauri() && isCommand(e, 'n')) {
      // ⌘N / ⌘O are routed by Rust's menu to the focused panel; the browser mock has no menu.
      e.preventDefault()
      void send({ type: 'newTicket' })
    } else if (!isTauri() && isCommand(e, 'o')) {
      e.preventDefault()
      void send({ type: 'openMain' })
    }
  })

  if (!menu) return null
  const content = menu.content
  const counter = timer?.running?.counter ?? null

  const row = (r: MenuRow) => (
    <MenuBarTicketRow
      key={r.ticketId}
      ticketRef={r.ref}
      title={r.title}
      needsYou={r.needsYou}
      runningLabel={r.right.type === 'running' ? (counter ?? r.right.counter) : null}
      total={r.right.type === 'spent' ? r.right.text : undefined}
      timer={r.button.type === 'slot' ? 'none' : r.button.type}
      timerDisabled={r.button.type === 'start' ? !r.button.enabled : false}
      timerTooltip={r.button.type === 'start' ? r.button.help : undefined}
      onToggleTimer={() =>
        void (r.button.type === 'stop' ? sendTimer({ type: 'stop' }) : sendTimer({ type: 'start', ticketId: r.ticketId }))
      }
      onOpen={() => void send({ type: 'openTicket', ticketId: r.ticketId })}
      onResolve={r.canResolve ? () => void send({ type: 'resolve', ticketId: r.ticketId }) : undefined}
    />
  )

  return (
    <div className="p-px">
      <MenuBarPanel className="rounded-[12px] ring-1 ring-line">
        <SearchRow query={menu.query} send={send} />
        <TimerNotice className="mx-2 mt-2" />
        {content.type === 'loading' ? (
          <LoadingState label="Loading tickets from iTop" className="py-8" />
        ) : content.type === 'failed' ? (
          <div className="flex flex-col items-center gap-2.5 px-5 py-6 text-center">
            <p role="alert" className="text-[12.5px] leading-snug text-danger">
              {content.text}
            </p>
            <Button size="sm" onClick={() => void send({ type: 'refresh' })}>
              Retry
            </Button>
          </div>
        ) : content.type === 'empty' ? (
          <p className="px-5 py-7 text-center text-[12.5px] leading-snug text-ink-2">{content.text}</p>
        ) : (
          <div className="kit-scroll overflow-y-auto pb-1.5" style={{ height: content.height }}>
            {content.running ? <MenuBarSection title="Running">{row(content.running)}</MenuBarSection> : null}
            {content.sections.map((s) => (
              <MenuBarSection key={s.group} title={s.title}>
                {s.rows.map(row)}
              </MenuBarSection>
            ))}
          </div>
        )}
        <MenuBarFooter
          onRefresh={() => void send({ type: 'refresh' })}
          refreshing={menu.refreshing}
          updatedLabel={menu.updatedText ?? undefined}
          onOpenApp={() => void send({ type: 'openMain' })}
        />
      </MenuBarPanel>
    </div>
  )
}

function SearchRow({ query, send }: { query: string; send: Send<'menubar'> }) {
  const [value, setValue] = useModelText(query, (q) => send({ type: 'setQuery', query: q }))
  return <MenuBarTopRow query={value} onQueryChange={setValue} onNewTicket={() => void send({ type: 'newTicket' })} />
}
