import { useLayoutEffect, useRef, useState } from 'react'
import {
  Button,
  ClassIcon,
  CopiedChip,
  MoreMenu,
  NeedsYouPill,
  StatusPill,
  TimerButton,
  Tooltip,
  type MenuEntry,
} from '@/components/kit'
import { openUrl, useModel, type ListScope, type TicketDetailView } from '@/backend'
import { cn } from '@/lib/utils'
import { useCloseConfirm } from '../sheets/closeConfirmStore'

/* ─────────────────────────────────────────────────────────
 * DETAIL HEADER (DET-7…24, TMR-19). First line: the ref (a quiet
 * mono button that copies it), the class icon, the status as text
 * and the teal "Needs you" pill. Then the title and the 2-second
 * "Copied" chip. Actions: timer, the one primary action, "…".
 * The actions sit right of the title, or on their own row under
 * it when the header is narrower than actions + 12 + 260 (DET-11).
 * ───────────────────────────────────────────────────────── */

export function DetailHeader({ scope, view }: { scope: ListScope; view: TicketDetailView }) {
  const [, send] = useModel(`detail:${scope}`)
  const [timer] = useModel('timer')
  const [, sendAssign] = useModel(`assign:${scope}`)
  const [, sendPending] = useModel(`pending:${scope}`)
  const [, sendResolve] = useModel(`resolve:${scope}`)
  const askClose = useCloseConfirm((s) => s.ask)
  const { ticket, primary, more, timerButton } = view

  const headerRef = useRef<HTMLDivElement>(null)
  const actionsRef = useRef<HTMLDivElement>(null)
  const [stacked, setStacked] = useState(false)

  useLayoutEffect(() => {
    const header = headerRef.current
    const actions = actionsRef.current
    if (!header || !actions || typeof ResizeObserver === 'undefined') return
    const measure = () => {
      const width = header.clientWidth
      setStacked(width > 0 && width < actions.scrollWidth + 12 + 260)
    }
    const observer = new ResizeObserver(measure)
    observer.observe(header)
    observer.observe(actions)
    measure()
    return () => observer.disconnect()
  }, [])

  const run = (p: Promise<unknown>) => void p.catch((error: unknown) => console.error('detail intent failed', error))

  const onPrimary = () => {
    if (primary.type !== 'action') return
    switch (primary.kind) {
      case 'assignToMe':
        return run(send({ type: 'assignToMe' }))
      case 'openAssign':
        return run(sendAssign({ type: 'open', ticketId: ticket.id }))
      case 'openResolve':
        return run(sendResolve({ type: 'open', ticketId: ticket.id }))
      case 'confirmClose':
        return askClose(scope, ticket.id)
    }
  }

  const moreItems: MenuEntry[] = [
    ...(more.assign
      ? [{ id: 'assign', label: more.assign.label, disabled: !more.enabled, onSelect: () => run(sendAssign({ type: 'open', ticketId: ticket.id })) }]
      : []),
    ...(more.pending
      ? [{ id: 'pending', label: 'Set to pending…', disabled: !more.enabled, onSelect: () => run(sendPending({ type: 'open', ticketId: ticket.id })) }]
      : []),
    ...(more.reopen ? [{ id: 'reopen', label: 'Reopen ticket', disabled: !more.enabled, onSelect: () => run(send({ type: 'reopen' })) }] : []),
    ...(more.divider ? [{ kind: 'separator' as const, id: 'sep' }] : []),
    { id: 'copy', label: `Copy ref ${ticket.ref}`, onSelect: () => run(send({ type: 'copyRef' })) },
    { id: 'web', label: 'Open in iTop on the web', onSelect: () => run(openUrl(more.webUrl)) },
  ]

  return (
    // Empty parts of the header move the window (the overlay title bar has no chrome of its own).
    <div ref={headerRef} data-tauri-drag-region className={cn('flex gap-3', stacked ? 'flex-col items-start gap-2.5' : 'items-start')}>
      <div data-tauri-drag-region className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[12px] text-ink-2">
          <Tooltip content={`Copy ${ticket.ref}`} describe={false}>
            <button
              type="button"
              onClick={() => run(send({ type: 'copyRef' }))}
              aria-label={`${ticket.ref}, copy ref`}
              className="mono-figures -mx-1 inline-flex h-6 items-center rounded-[5px] px-1 text-[12px] text-ink-2 transition-colors duration-100 hover:bg-hover-2 hover:text-ink"
            >
              {ticket.ref}
            </button>
          </Tooltip>
          <ClassIcon kind={ticket.ticketClass} />
          <StatusPill>{ticket.statusLabel}</StatusPill>
          {ticket.needsYou ? <NeedsYouPill /> : null}
          {view.copied ? <CopiedChip className="h-6">Copied {ticket.ref}</CopiedChip> : null}
        </div>
        <h2 className="mt-1 select-text text-[22px] font-semibold leading-[1.25] tracking-[-0.01em] text-ink [overflow-wrap:anywhere]">
          {ticket.title}
        </h2>
      </div>

      <div ref={actionsRef} className="flex w-max shrink-0 items-center gap-2 pt-0.5">
        {timerButton.type === 'running' ? (
          <TimerButton state="running" counter={timer?.running?.counter ?? undefined} onToggle={() => run(send({ type: 'toggleTimer' }))} />
        ) : timerButton.type === 'webStopwatch' ? (
          <TimerButton state="web" label={timerButton.label} />
        ) : timerButton.type === 'start' ? (
          <TimerButton
            state="idle"
            disabled={!timerButton.enabled}
            tooltip={timerButton.help}
            onToggle={() => run(send({ type: 'toggleTimer' }))}
          />
        ) : null}
        {primary.type === 'action' ? (
          <Button
            variant={primary.teal ? 'need' : 'primary'}
            size="sm"
            busy={primary.busy}
            disabled={!primary.enabled}
            tooltip={primary.help}
            onClick={onPrimary}
            className="h-[26px] rounded-[7px] px-3 text-[12.5px] font-semibold"
          >
            {primary.label}
          </Button>
        ) : null}
        <MoreMenu items={moreItems} />
      </div>
    </div>
  )
}
