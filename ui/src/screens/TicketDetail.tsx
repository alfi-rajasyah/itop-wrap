import { useEffect } from 'react'
import {
  AttachmentChip,
  BuildingIcon,
  CIChip,
  DetailItem,
  DetailStrip,
  InitialsAvatar,
  LockIcon,
  PersonIcon,
  SectionHeading,
  SegmentedControl,
  StopwatchIcon,
  TeamIcon,
  Tooltip,
  UnassignedMark,
} from '@/components/kit'
import { useModel, type AgentChip, type Attachments, type DetailBody, type ListScope, type LogEntryView, type TicketDetailView } from '@/backend'
import { DetailPlaceholder, InlineError, InlineLoading } from '@/lib/paneStates'
import { DetailComposer } from './detail/DetailComposer'
import { DetailHeader } from './detail/DetailHeader'
import { RichText } from './detail/RichText'

/* ─────────────────────────────────────────────────────────
 * TICKET DETAIL (section 4, 4b, 5 of the parity checklist): the
 * selected ticket of one list. Header and strip show at once from
 * the list row; service line, description, attachments, logs and
 * the composer appear once loaded. Everything shown is the
 * `detail:<scope>` snapshot; ⌘T toggles the timer when the
 * snapshot offers it.
 * ───────────────────────────────────────────────────────── */

export default function TicketDetail({ scope }: { scope: 'mine' | 'team' }) {
  const [detail, send] = useModel(`detail:${scope}`)

  // ⌘T (KEY-4): only when the timer button is there, and not while a sheet is up.
  const timerToggle = detail?.type === 'ticket' && (detail.timerButton.type === 'running' || (detail.timerButton.type === 'start' && detail.timerButton.enabled))
  useEffect(() => {
    if (!timerToggle) return
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey || e.key.toLowerCase() !== 't') return
      if (document.querySelector('[aria-modal="true"]')) return
      e.preventDefault()
      void send({ type: 'toggleTimer' }).catch((error: unknown) => console.error('toggleTimer failed', error))
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [timerToggle, send])

  if (!detail) return <div className="flex-1" />
  if (detail.type === 'none') return <DetailPlaceholder text={detail.placeholder} />
  return <TicketView key={detail.ticket.id} scope={scope} view={detail} />
}

function TicketView({ scope, view }: { scope: ListScope; view: TicketDetailView }) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="kit-scroll min-h-0 flex-1 overflow-y-auto">
        <article aria-label={`${view.ticket.ref} ${view.ticket.title}`} className="flex max-w-[860px] flex-col gap-4 px-6 pb-8 pt-5">
          <DetailHeader scope={scope} view={view} />
          <Strip view={view} />
          <Body scope={scope} view={view} body={view.body} />
        </article>
      </div>
      {view.composer ? <DetailComposer scope={scope} ticketId={view.ticket.id} composer={view.composer} /> : null}
    </div>
  )
}

// ── Details strip (DET-25…29) ──────────────────────────────────────────────────────────────

function Strip({ view }: { view: TicketDetailView }) {
  const [, sendShell] = useModel('shell')
  const { strip } = view
  return (
    <DetailStrip>
      <DetailItem icon={<PersonIcon />} label="Caller">
        {strip.caller}
      </DetailItem>
      <DetailItem icon={<BuildingIcon />} label="Organization">
        {strip.organization}
      </DetailItem>
      <DetailItem icon={<TeamIcon />} label="Team">
        {strip.team}
      </DetailItem>
      <Agent chip={strip.agent} />
      {strip.spent ? (
        <DetailItem icon={<StopwatchIcon />} label="Time spent" className="mono-figures text-[12px]">
          {strip.spent}
        </DetailItem>
      ) : null}
      {strip.linkedCIs.map((ci) => (
        <CIChip
          key={`${ci.className}::${ci.key}`}
          variant="link"
          name={ci.name}
          tooltip={`Linked CI: open ${ci.name} in Config items`}
          onClick={() => void sendShell({ type: 'openCI', ci })}
        />
      ))}
    </DetailStrip>
  )
}

function Agent({ chip }: { chip: AgentChip }) {
  if (chip.type === 'unassigned') {
    return (
      <Tooltip content="Agent" describe={false}>
        <span role="img" aria-label="Agent: unassigned" className="inline-flex items-center gap-1.5 text-[12.5px] font-semibold text-ink">
          <UnassignedMark size={18} />
          Unassigned
        </span>
      </Tooltip>
    )
  }
  return (
    <Tooltip content="Agent" describe={false}>
      <span role="img" aria-label={`Agent: ${chip.name}`} className="inline-flex min-w-0 items-center gap-1.5 text-[12.5px] text-ink">
        <InitialsAvatar name={chip.name} initials={chip.initials} size={18} />
        <span className="truncate">{chip.name}</span>
      </span>
    </Tooltip>
  )
}

// ── Body: loading / failed / loaded (DET-4…6, DET-30…39) ───────────────────────────────────

function Body({ scope, view, body }: { scope: ListScope; view: TicketDetailView; body: DetailBody }) {
  const [, send] = useModel(`detail:${scope}`)
  if (body.type === 'loading') return <InlineLoading>Loading the ticket from iTop</InlineLoading>
  if (body.type === 'failed') return <InlineError onRetry={() => void send({ type: 'retry' })}>{body.text}</InlineError>
  return (
    <>
      {body.serviceLine ? <p className="-mt-1.5 text-[12px] text-ink-2">{body.serviceLine}</p> : null}

      <section aria-labelledby={`desc-${scope}`} className="flex flex-col gap-2">
        <h3 id={`desc-${scope}`} className="text-[12.5px] font-semibold text-ink">
          Description
        </h3>
        {body.description ? <RichText content={body.description} /> : <p className="text-[13px] text-ink-2">No description.</p>}
        <AttachmentsView scope={scope} attachments={view.attachments} />
      </section>

      <section aria-label="Logs" className="mt-2 flex flex-col gap-3">
        <SegmentedControl
          label="Log"
          size="sm"
          className="w-max"
          options={[
            { value: 'public', label: 'Public log' },
            { value: 'private', label: 'Private log' },
          ]}
          value={body.tab}
          onChange={(tab) => void send({ type: 'setTab', tab })}
        />
        {body.emptyLogText ? <p className="text-[13px] text-ink-2">{body.emptyLogText}</p> : null}
        {body.entries.length > 0 ? (
          <ol className="flex flex-col gap-3">
            {body.entries.map((entry) => (
              <li key={entry.id}>
                <LogCard entry={entry} />
              </li>
            ))}
          </ol>
        ) : null}
      </section>
    </>
  )
}

/** One case-log entry (DET-37): a card with "<author> · <date>", a lock on private ones. */
function LogCard({ entry }: { entry: LogEntryView }) {
  const at = entry.header.indexOf(' · ')
  const author = at >= 0 ? entry.header.slice(0, at) : entry.header
  const date = at >= 0 ? entry.header.slice(at + 3) : null
  return (
    <article className="rounded-card bg-surface shadow-card">
      <header className="flex items-center gap-1.5 border-b border-line px-3.5 py-2 text-[12px] text-ink-2">
        {entry.isPrivate ? <LockIcon role="img" aria-label="Private" className="size-3.5 shrink-0" /> : null}
        {author ? <span className="font-medium text-ink">{author}</span> : null}
        {date ? (
          <>
            <span aria-hidden>·</span>
            <span className="tabular-nums">{date}</span>
          </>
        ) : null}
      </header>
      <RichText content={entry.content} className="px-3.5 pb-3 pt-2.5" />
    </article>
  )
}

// ── Attachments (DET-32…34) ────────────────────────────────────────────────────────────────

function AttachmentsView({ scope, attachments }: { scope: ListScope; attachments: Attachments }) {
  const [, send] = useModel(`detail:${scope}`)
  if (attachments.type === 'loading') return null
  if (attachments.type === 'failed') return <p className="text-[12px] text-ink-2">{attachments.text}</p>
  if (attachments.items.length === 0) return null
  const busy = attachments.downloadingId !== null
  return (
    <div className="mt-2 flex flex-col gap-1.5">
      <SectionHeading>Attachments</SectionHeading>
      <div className="flex flex-wrap gap-1.5">
        {attachments.items.map((a) => (
          <AttachmentChip
            key={a.id}
            name={a.name}
            busy={attachments.downloadingId === a.id}
            disabled={busy}
            onClick={() => void send({ type: 'downloadAttachment', attachmentId: a.id })}
          />
        ))}
      </div>
      <div aria-live="polite" className="text-[12px]">
        {attachments.saved ? <p className="text-ink-2">{attachments.saved}</p> : null}
        {attachments.error ? <p className="text-danger">{attachments.error}</p> : null}
      </div>
    </div>
  )
}
