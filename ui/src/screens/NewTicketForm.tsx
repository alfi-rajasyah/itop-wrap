import { useCallback, type KeyboardEvent } from 'react'
import { Button, Dialog } from '@/components/kit'
import { useModel, type NewTicketIntent, type NewTicketOpen } from '@/backend'
import { NewTicketFields, NewTicketMessages } from './newticket/NewTicketFields'

/* ─────────────────────────────────────────────────────────
 * NEW TICKET (section 8). `sheet`: over the main window while
 * `newTicket:sheet` is open. `window`: fills the separate New
 * Ticket window while `newTicket:window` is open. Cancel / Esc
 * keeps a draft (Rust decides), ⌘↩ creates, "Clear form" shows
 * whenever a draft exists. The form closes when Rust says so.
 * ───────────────────────────────────────────────────────── */

export default function NewTicketForm({ instance }: { instance: 'sheet' | 'window' }) {
  const [form, send] = useModel(`newTicket:${instance}`)
  const dispatch = useCallback(
    (intent: NewTicketIntent) => send(intent) as Promise<unknown>,
    [send],
  )
  if (form?.type !== 'open') return null
  return instance === 'sheet' ? <NewTicketSheet form={form} send={dispatch} /> : <NewTicketWindow form={form} send={dispatch} />
}

type Props = { form: NewTicketOpen; send: (intent: NewTicketIntent) => Promise<unknown> }

function run(send: Props['send'], intent: NewTicketIntent) {
  void send(intent).catch((error: unknown) => console.error('newTicket intent failed', error))
}

function ClearForm({ form, send }: Props) {
  if (!form.showClearForm) return null
  return (
    <Button variant="quiet" size="sm" onClick={() => run(send, { type: 'clearForm' })}>
      Clear form
    </Button>
  )
}

function NewTicketSheet({ form, send }: Props) {
  return (
    <Dialog
      open
      width={480}
      title="New ticket"
      subtitle={form.subtitle ?? undefined}
      onCancel={() => run(send, { type: 'cancel' })}
      onConfirm={() => run(send, { type: 'create' })}
      confirmLabel="Create ticket"
      confirmDisabled={!form.confirmEnabled}
      confirmBusy={form.creating}
      error={<NewTicketMessages form={form} />}
      footerStart={<ClearForm form={form} send={send} />}
    >
      <NewTicketFields form={form} send={send} />
    </Dialog>
  )
}

/** The separate window (NEW-2): the same form, filling the window, buttons at the bottom. */
function NewTicketWindow({ form, send }: Props) {
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault()
      run(send, { type: 'cancel' })
    } else if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      if (form.confirmEnabled && !form.creating) run(send, { type: 'create' })
    }
  }
  return (
    <div onKeyDown={onKeyDown} className="flex h-full min-h-0 flex-col bg-surface text-ink">
      <header data-tauri-drag-region className="shrink-0 px-5 pb-1 pt-4">
        <h1 data-tauri-drag-region className="text-[15px] font-semibold leading-snug">
          New ticket
        </h1>
        {form.subtitle ? <p className="mt-0.5 text-[13px] text-ink-2">{form.subtitle}</p> : null}
      </header>
      <div className="kit-scroll min-h-0 flex-1 overflow-y-auto px-5 pb-2 pt-3">
        <NewTicketFields form={form} send={send} />
      </div>
      <div className="min-h-[18px] shrink-0 px-5 text-[12px] leading-snug text-danger" aria-live="polite">
        <NewTicketMessages form={form} />
      </div>
      <footer className="flex shrink-0 items-center gap-2 px-5 pb-4 pt-2">
        <div className="mr-auto flex items-center">
          <ClearForm form={form} send={send} />
        </div>
        <Button variant="secondary" size="md" onClick={() => run(send, { type: 'cancel' })}>
          Cancel
        </Button>
        <Button
          variant="primary"
          size="md"
          disabled={!form.confirmEnabled}
          busy={form.creating}
          onClick={() => run(send, { type: 'create' })}
          aria-keyshortcuts="Meta+Enter"
        >
          Create ticket
        </Button>
      </footer>
    </div>
  )
}
