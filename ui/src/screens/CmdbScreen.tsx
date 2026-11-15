import { useCallback } from 'react'
import {
  Button,
  CIChip,
  CopiedChip,
  CopyIcon,
  ExternalIcon,
  IconButton,
  KeyValueGrid,
  Listbox,
  MasterDetail,
  NeedsYouDot,
  NewTicketIcon,
  PaneHeader,
  Ref,
  SearchField,
  SectionHeader,
  SimpleRow,
  Tooltip,
} from '@/components/kit'
import { openUrl, useModel, type CIDetailView, type CIChip as CIChipView, type CIResults } from '@/backend'
import { DetailPlaceholder, InlineError, InlineLoading, PaneLoading, PaneMessage } from '@/lib/paneStates'
import { ScopeBar } from '@/lib/ScopeBar'
import { useModelText } from '@/lib/useModelText'

/* ─────────────────────────────────────────────────────────
 * CONFIG ITEMS (section 11): search (or an org's CIs) on the left,
 * the CI on the right with its fields, relationships and open
 * tickets. Chips open another CI here; a ticket row goes to the
 * ticket lists through the shell (SHELL-9).
 * ───────────────────────────────────────────────────────── */

export default function CmdbScreen(_props: object) {
  return <MasterDetail listLabel="Resize the config item list" list={<CmdbList />} detail={<CmdbDetail />} />
}

function CmdbList() {
  const [cmdb, send] = useModel('cmdb')
  const [shell] = useModel('shell')
  const commit = useCallback((query: string) => send({ type: 'setQuery', query }), [send])
  const [query, setQuery] = useModelText(cmdb?.query ?? '', commit)
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PaneHeader title={shell?.contentTitle ?? 'Config items'}>
        <SearchField value={query} onChange={setQuery} placeholder="Search config items" />
      </PaneHeader>
      {cmdb?.scope ? (
        <ScopeBar text={`Config items in ${cmdb.scope.orgName}`} onClear={() => void send({ type: 'clearScope' })} />
      ) : null}
      {cmdb ? <Results results={cmdb.results} selectedId={cmdb.selectedId} onSelect={(id) => void send({ type: 'select', id })} /> : null}
    </div>
  )
}

function Results({ results, selectedId, onSelect }: { results: CIResults; selectedId: string | null; onSelect: (id: string) => void }) {
  switch (results.type) {
    case 'idle':
      return <PaneMessage>{results.text}</PaneMessage>
    case 'searching':
      return <PaneLoading label="Searching iTop" />
    case 'failed':
      return <PaneMessage tone="error">{results.text}</PaneMessage>
    case 'empty':
      return <PaneMessage>{results.text}</PaneMessage>
    case 'loaded':
      return (
        <div className="kit-scroll min-h-0 flex-1 overflow-y-auto">
          <SectionHeader title={results.header} />
          <Listbox label="Config items" selectedId={selectedId} onSelect={onSelect} className="gap-px px-1.5 pb-2">
            {results.items.map((ci) => (
              <SimpleRow key={ci.id} id={ci.id} title={ci.name} subtitle={ci.subtitle} mono />
            ))}
          </Listbox>
        </div>
      )
  }
}

function CmdbDetail() {
  const [cmdb] = useModel('cmdb')
  if (!cmdb) return <div className="flex-1" />
  if (!cmdb.current) return <DetailPlaceholder text={cmdb.placeholder} />
  return <CIView key={cmdb.current.id} ci={cmdb.current} />
}

function CIView({ ci }: { ci: CIDetailView }) {
  const [, send] = useModel('cmdb')
  const [, sendShell] = useModel('shell')
  const body = ci.body
  return (
    <div className="kit-scroll min-h-0 flex-1 overflow-y-auto">
      <article aria-label={ci.name} className="flex max-w-[860px] flex-col gap-5 px-6 pb-8 pt-5">
        <header className="flex items-start gap-3">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <p className="text-[12px] text-ink-2">{ci.headerLine}</p>
              {ci.copied ? <CopiedChip className="h-6">Copied {ci.name}</CopiedChip> : null}
            </div>
            <h2 className="mono-figures mt-1 select-text text-[22px] font-semibold leading-[1.25] text-ink [overflow-wrap:anywhere]">{ci.name}</h2>
          </div>
          <div className="flex shrink-0 items-center gap-1.5 pt-0.5">
            <Button
              size="sm"
              variant="secondary"
              icon={<NewTicketIcon />}
              tooltip={`New ticket on ${ci.name}`}
              onClick={() => void sendShell({ type: 'newTicketForCI', key: ci.key, name: ci.name, orgName: ci.orgName })}
              className="h-[26px] rounded-[7px] text-[12.5px]"
            >
              New ticket
            </Button>
            <IconButton label="Copy name" icon={<CopyIcon />} variant="field" size="sm" onClick={() => void send({ type: 'copyName' })} />
            <IconButton label="Open in iTop on the web" icon={<ExternalIcon />} variant="field" size="sm" onClick={() => void openUrl(ci.webUrl)} />
          </div>
        </header>

        {body.type === 'loading' ? <InlineLoading>{body.text}</InlineLoading> : null}
        {body.type === 'failed' ? <InlineError onRetry={() => void send({ type: 'retryDetail' })}>{body.text}</InlineError> : null}
        {body.type === 'loaded' ? (
          <>
            {body.fields.length > 0 ? (
              <div className="rounded-card bg-inset px-4 py-3.5 shadow-hairline">
                <KeyValueGrid items={body.fields.map((f) => ({ key: f.label, label: f.label, value: <span className="select-text">{f.value}</span> }))} />
              </div>
            ) : null}

            {body.description ? (
              <section className="flex flex-col gap-1.5">
                <h3 className="text-[12.5px] font-semibold text-ink">Description</h3>
                <p className="max-w-[72ch] select-text whitespace-pre-wrap text-[13px] leading-[1.6] text-ink">{body.description}</p>
              </section>
            ) : null}

            <section className="flex flex-col gap-2.5">
              <h3 className="text-[14px] font-semibold text-ink">Relationships</h3>
              <div className="grid grid-cols-[repeat(auto-fit,minmax(220px,1fr))] gap-x-6 gap-y-4">
                <div className="flex min-w-0 flex-col gap-1.5">
                  <h4 className="text-[12px] text-ink-2">Depends on</h4>
                  {body.dependsOn.length === 0 ? <p className="text-[13px] text-ink-2">Nothing listed in the CMDB.</p> : <Chips chips={body.dependsOn} />}
                </div>
                <div className="flex min-w-0 flex-col gap-1.5">
                  <h4 className="text-[12px] text-ink-2">Used by</h4>
                  {body.usedBy.length > 0 ? <Chips chips={body.usedBy} /> : null}
                  <p className="text-[12px] leading-[1.5] text-ink-2">{body.usedBySentence}</p>
                </div>
              </div>
            </section>

            <section className="flex flex-col gap-2">
              <h3 className="flex items-baseline gap-1.5 text-[14px] font-semibold text-ink">
                Open tickets on this CI <span className="text-[13px] font-normal tabular-nums text-ink-2">{body.openTickets.length}</span>
              </h3>
              {body.openTicketsEmptyText ? <p className="text-[13px] text-ink-2">{body.openTicketsEmptyText}</p> : null}
              {body.openTickets.length > 0 ? (
                <ul className="flex flex-col gap-1.5">
                  {body.openTickets.map((t) => (
                    <li key={t.ticketId}>
                      <Tooltip content={`Open ${t.ref}`} describe={false} className="flex">
                        <button
                          type="button"
                          onClick={() => void sendShell({ type: 'openTicket', ticket: { ticketId: t.ticketId, ref: t.ref } })}
                          className="flex w-full min-w-0 items-center gap-2.5 rounded-card bg-surface px-3 py-2.5 text-left shadow-card transition-colors duration-100 hover:bg-hover"
                        >
                          {t.needsYou ? <NeedsYouDot /> : null}
                          <Ref>{t.ref}</Ref>
                          <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-ink">{t.title}</span>
                          <span className="shrink-0 truncate text-[12px] text-ink-2">{t.statusLine}</span>
                        </button>
                      </Tooltip>
                    </li>
                  ))}
                </ul>
              ) : null}
            </section>
          </>
        ) : null}
      </article>
    </div>
  )
}

function Chips({ chips }: { chips: CIChipView[] }) {
  const [, send] = useModel('cmdb')
  return (
    <div className="flex flex-wrap gap-1.5">
      {chips.map((c) => (
        <CIChip
          key={`${c.className}::${c.key}`}
          name={c.name}
          tooltip={c.help}
          onClick={() => void send({ type: 'openCI', key: c.key, className: c.className, name: c.name })}
        />
      ))}
    </div>
  )
}
