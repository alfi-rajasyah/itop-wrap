import { useCallback, useEffect, type KeyboardEvent } from 'react'
import { Button, Dialog, Field, StopwatchIcon, TextArea } from '@/components/kit'
import {
  useModel,
  type AssignSheetOpen,
  type ListScope,
  type PendingSheetOpen,
  type ResolveSheetIntent,
  type ResolveSheetOpen,
} from '@/backend'
import { SnapshotDropdown } from '@/lib/SnapshotDropdown'
import { useModelText } from '@/lib/useModelText'
import { useCloseConfirm } from './sheets/closeConfirmStore'

/* ─────────────────────────────────────────────────────────
 * TICKET SHEETS (sections 6 and 7): Assign, Set to pending and
 * Resolve, each shown exactly while its model is `open`, plus the
 * "Close <ref>?" confirmation the detail asks for. Esc = Cancel
 * (an open dropdown takes Esc first), ⌘↩ = the confirm button.
 * Rust closes a sheet itself on success; React never infers it.
 * ───────────────────────────────────────────────────────── */

function logFailure(error: unknown) {
  console.error('sheet intent failed', error)
}

export default function TicketSheets({ scope }: { scope: 'mine' | 'team' }) {
  const [assign] = useModel(`assign:${scope}`)
  const [pending] = useModel(`pending:${scope}`)
  const [resolve] = useModel(`resolve:${scope}`)
  return (
    <>
      {assign?.type === 'open' ? <AssignSheet scope={scope} sheet={assign} /> : null}
      {pending?.type === 'open' ? <PendingSheet scope={scope} sheet={pending} /> : null}
      {resolve?.type === 'open' ? <ResolveSheet scope={scope} sheet={resolve} /> : null}
      <CloseConfirm scope={scope} />
    </>
  )
}

function TimerLine({ text }: { text: string | null }) {
  if (!text) return null
  return (
    <p className="flex items-center gap-1.5 text-[12px] text-ink-2">
      <StopwatchIcon aria-hidden className="size-3.5 shrink-0" />
      {text}
    </p>
  )
}

// ── Assign (ASG-6…13) ──────────────────────────────────────────────────────────────────────

function AssignSheet({ scope, sheet }: { scope: ListScope; sheet: AssignSheetOpen }) {
  const [, send] = useModel(`assign:${scope}`)
  return (
    <Dialog
      open
      width={380}
      title={sheet.title}
      subtitle={sheet.subtitle}
      onCancel={() => void send({ type: 'close' }).catch(logFailure)}
      onConfirm={() => void send({ type: 'confirm' }).catch(logFailure)}
      confirmLabel={sheet.confirmLabel}
      confirmDisabled={!sheet.confirmEnabled}
      confirmBusy={sheet.busy}
      error={sheet.error}
    >
      <Button
        variant="primary"
        size="block"
        disabled={!sheet.assignToMe.enabled}
        tooltip={sheet.assignToMe.help}
        onClick={() => void send({ type: 'assignToMe' }).catch(logFailure)}
      >
        Assign to me
      </Button>
      <p className="py-1.5 text-center text-[12px] text-ink-2">or choose a team and agent</p>
      <Field label="Team" reserveErrorSpace={false}>
        {({ id }) => (
          <SnapshotDropdown id={id} aria-labelledby={`${id}-label`} dropdown={sheet.team} onPick={(teamId) => void send({ type: 'setTeam', teamId })} />
        )}
      </Field>
      <Field label="Agent" reserveErrorSpace={false} className="mt-1.5">
        {({ id }) => (
          <SnapshotDropdown id={id} aria-labelledby={`${id}-label`} dropdown={sheet.agent} onPick={(agentId) => void send({ type: 'setAgent', agentId })} />
        )}
      </Field>
    </Dialog>
  )
}

// ── Set to pending (LC-9…12) ───────────────────────────────────────────────────────────────

function PendingSheet({ scope, sheet }: { scope: ListScope; sheet: PendingSheetOpen }) {
  const [, send] = useModel(`pending:${scope}`)
  const commit = useCallback((reason: string) => send({ type: 'setReason', reason }), [send])
  const [reason, setReason] = useModelText(sheet.reason, commit, sheet.ticketId)
  return (
    <Dialog
      open
      title={sheet.title}
      subtitle={sheet.subtitle}
      onCancel={() => void send({ type: 'close' }).catch(logFailure)}
      onConfirm={() => void send({ type: 'confirm' }).catch(logFailure)}
      confirmLabel="Set to pending"
      confirmDisabled={!sheet.confirmEnabled}
      confirmBusy={sheet.busy}
      error={sheet.error}
    >
      <Field label="Reason" required requiredError={sheet.reasonRequired.missing} error={sheet.reasonError}>
        {({ id, describedBy }) => (
          <TextArea
            id={id}
            data-autofocus
            aria-required
            aria-describedby={describedBy}
            minHeight={90}
            placeholder="What are you waiting for? For example the caller's reply or a part."
            value={reason}
            onChange={(e) => setReason(e.target.value)}
          />
        )}
      </Field>
      <TimerLine text={sheet.timerLine} />
    </Dialog>
  )
}

// ── Resolve (LC-13…21) ─────────────────────────────────────────────────────────────────────

/** What the Resolve form sends: the same intents for `resolve:<scope>` and `resolve:window`. */
type ResolveSend = (intent: ResolveSheetIntent) => Promise<unknown>

function ResolveSheet({ scope, sheet }: { scope: ListScope; sheet: ResolveSheetOpen }) {
  const [, send] = useModel(`resolve:${scope}`)
  return (
    <Dialog
      open
      title={sheet.title}
      subtitle={sheet.subtitle}
      onCancel={() => void send({ type: 'close' }).catch(logFailure)}
      onConfirm={() => void send({ type: 'confirm' }).catch(logFailure)}
      confirmLabel="Resolve ticket"
      confirmDisabled={!sheet.confirmEnabled}
      confirmBusy={sheet.busy}
      error={sheet.error}
      width={500}
      footer={<ResolveButtons sheet={sheet} send={send} />}
    >
      <ResolveFields sheet={sheet} send={send} />
    </Dialog>
  )
}

/**
 * The menu bar's Resolve window (index.html?window=resolve): the same form as the sheet, filling
 * its own titled window. Esc = Cancel, ⌘↩ = Resolve ticket. Rust closes the window when the
 * form closes.
 */
export function ResolveWindowForm() {
  const [sheet, send] = useModel('resolve:window')
  if (sheet?.type !== 'open') return null
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault()
      void send({ type: 'close' }).catch(logFailure)
    } else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      if (sheet.confirmEnabled && !sheet.busy) void send({ type: 'confirm' }).catch(logFailure)
    }
  }
  return (
    <div onKeyDown={onKeyDown} className="flex flex-col bg-surface text-ink">
      <header className="shrink-0 px-5 pb-1 pt-4">
        <h1 className="text-[15px] font-semibold leading-snug">{sheet.title}</h1>
        <p className="mt-0.5 line-clamp-2 text-[13px] leading-snug text-ink-2">{sheet.subtitle}</p>
      </header>
      <div className="flex flex-col gap-1.5 px-5 pb-1 pt-3">
        <ResolveFields sheet={sheet} send={send} />
      </div>
      <div className="min-h-[18px] px-5" aria-live="polite">
        {sheet.error ? <p className="text-[12px] leading-snug text-danger">{sheet.error}</p> : null}
      </div>
      <div className="px-5 pb-4 pt-2">
        <ResolveButtons sheet={sheet} send={send} />
      </div>
    </div>
  )
}

/** Three equal buttons, centred (owner's call): Cancel, Resolve and Close, Resolve ticket. */
function ResolveButtons({ sheet, send }: { sheet: ResolveSheetOpen; send: ResolveSend }) {
  const button = 'h-8 w-full whitespace-nowrap px-4'
  return (
    // auto-cols-fr: every button as wide as the widest label, on one line.
    <div className="flex w-full justify-center">
      <div className="inline-grid grid-flow-col auto-cols-fr gap-3">
        <Button variant="secondary" size="sm" className={button} onClick={() => void send({ type: 'close' }).catch(logFailure)}>
          Cancel
        </Button>
        <Button
          variant="danger"
          size="sm"
          className={button}
          disabled={!sheet.confirmEnabled}
          tooltip="Resolves the ticket, then closes it. A closed ticket can't be reopened from the app."
          onClick={() => void send({ type: 'confirmAndClose' }).catch(logFailure)}
        >
          Resolve and Close
        </Button>
        <Button
          variant="primary"
          size="sm"
          className={button}
          disabled={!sheet.confirmEnabled}
          busy={sheet.busy}
          aria-keyshortcuts="Meta+Enter"
          onClick={() => void send({ type: 'confirm' }).catch(logFailure)}
        >
          Resolve ticket
        </Button>
      </div>
    </div>
  )
}

function ResolveFields({ sheet, send }: { sheet: ResolveSheetOpen; send: ResolveSend }) {
  const commit = useCallback((solution: string) => send({ type: 'setSolution', solution }), [send])
  const [solution, setSolution] = useModelText(sheet.solution, commit, sheet.ticketId)
  return (
    <>
      <Field label="Service" required requiredError={sheet.serviceRequired.missing} reserveErrorSpace={false}>
        {({ id }) => (
          <SnapshotDropdown
            id={id}
            aria-labelledby={`${id}-label`}
            aria-required
            dropdown={sheet.service}
            onPick={(serviceId) => void send({ type: 'setService', serviceId })}
          />
        )}
      </Field>
      <Field label="Service subcategory" required requiredError={sheet.subcategoryRequired.missing} reserveErrorSpace={false} className="mt-2">
        {({ id }) => (
          <SnapshotDropdown
            id={id}
            aria-labelledby={`${id}-label`}
            aria-required
            dropdown={sheet.subcategory}
            onPick={(subcategoryId) => void send({ type: 'setSubcategory', subcategoryId })}
          />
        )}
      </Field>
      <Field label="Resolution code" reserveErrorSpace={false} className="mt-2">
        {({ id }) => (
          <SnapshotDropdown id={id} aria-labelledby={`${id}-label`} dropdown={sheet.code} onPick={(codeId) => void send({ type: 'setCode', codeId })} />
        )}
      </Field>
      <Field label="Solution" required requiredError={sheet.solutionRequired.missing} error={sheet.solutionError} className="mt-2">
        {({ id, describedBy }) => (
          <TextArea
            id={id}
            data-autofocus
            aria-required
            aria-describedby={describedBy}
            minHeight={110}
            placeholder="What fixed it, in words the caller understands."
            value={solution}
            onChange={(e) => setSolution(e.target.value)}
          />
        )}
      </Field>
      <TimerLine text={sheet.timerLine} />
    </>
  )
}

// ── Close confirmation (DET-16) ────────────────────────────────────────────────────────────

function CloseConfirm({ scope }: { scope: ListScope }) {
  const request = useCloseConfirm((s) => s.request)
  const dismiss = useCloseConfirm((s) => s.dismiss)
  const [detail, send] = useModel(`detail:${scope}`)
  const asked = request?.scope === scope
  const view = detail?.type === 'ticket' && detail.ticket.id === request?.ticketId ? detail : null

  // The ticket changed or went away underneath the question: drop it.
  useEffect(() => {
    if (asked && detail && !view) dismiss()
  }, [asked, detail, view, dismiss])

  if (!asked || !view) return null
  const confirm = () => {
    dismiss()
    void send({ type: 'close' }).catch(logFailure)
  }
  return (
    <Dialog
      open
      role="alertdialog"
      width={360}
      title={view.closeConfirm.title}
      subtitle={view.closeConfirm.message}
      onCancel={dismiss}
      onConfirm={confirm}
      footer={
        <>
          <span className="flex-1" />
          <Button variant="secondary" size="md" onClick={dismiss}>
            Cancel
          </Button>
          {/* The default button, as in the macOS confirmation: Return closes, Esc cancels. */}
          <Button variant="primary" size="md" data-autofocus onClick={confirm} aria-keyshortcuts="Meta+Enter">
            Close ticket
          </Button>
        </>
      }
    />
  )
}
