import { useCallback } from 'react'
import {
  Button,
  Listbox,
  MasterDetail,
  NewTicketIcon,
  PaneHeader,
  PeopleIcon,
  SearchField,
  ServerIcon,
  SimpleRow,
  Tooltip,
} from '@/components/kit'
import { useModel, type OrgDetailView, type OrgList } from '@/backend'
import { DetailPlaceholder, PaneLoading, PaneMessage } from '@/lib/paneStates'
import { TableStateView } from '@/lib/TableStateView'
import { useModelText } from '@/lib/useModelText'
import { ViewMenu, ViewOptionsSummary } from '@/lib/ViewMenu'

/* ─────────────────────────────────────────────────────────
 * ORGANIZATIONS (section 12): every active org with its open-ticket
 * count, filtered locally; the org on the right with links to its
 * people, config items and a new ticket, and its tickets grouped
 * by status under the shared View menu.
 * ───────────────────────────────────────────────────────── */

export default function OrgsScreen(_props: object) {
  return <MasterDetail listLabel="Resize the organization list" list={<OrgsList />} detail={<OrgsDetail />} />
}

function OrgsList() {
  const [orgs, send] = useModel('orgs')
  const [shell] = useModel('shell')
  const commit = useCallback((query: string) => send({ type: 'setQuery', query }), [send])
  const [query, setQuery] = useModelText(orgs?.query ?? '', commit)
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PaneHeader title={shell?.contentTitle ?? 'Organizations'}>
        <SearchField value={query} onChange={setQuery} placeholder="Search organizations" />
      </PaneHeader>
      {orgs ? <List list={orgs.list} selectedId={orgs.selectedId} /> : null}
    </div>
  )
}

function List({ list, selectedId }: { list: OrgList; selectedId: string | null }) {
  const [, send] = useModel('orgs')
  switch (list.type) {
    case 'loading':
      return <PaneLoading label="Loading organizations from iTop" />
    case 'failed':
      return (
        <PaneMessage
          tone="error"
          action={
            <Button size="sm" variant="secondary" onClick={() => void send({ type: 'retryList' })}>
              Retry
            </Button>
          }
        >
          {list.text}
        </PaneMessage>
      )
    case 'empty':
      return <PaneMessage>{list.text}</PaneMessage>
    case 'loaded':
      return (
        <div className="kit-scroll min-h-0 flex-1 overflow-y-auto">
          <Listbox label="Organizations" selectedId={selectedId} onSelect={(id) => void send({ type: 'select', id })} className="gap-px px-1.5 py-1">
            {list.orgs.map((org) => (
              <SimpleRow
                key={org.id}
                id={org.id}
                title={org.name}
                subtitle={org.code || undefined}
                trailing={
                  <Tooltip content={`${org.openCount} open tickets`} describe={false}>
                    <span aria-label={`${org.openCount} open tickets`} className="tabular-nums">
                      {org.openCount}
                    </span>
                  </Tooltip>
                }
              />
            ))}
          </Listbox>
        </div>
      )
  }
}

function OrgsDetail() {
  const [orgs] = useModel('orgs')
  if (!orgs) return <div className="flex-1" />
  if (!orgs.detail) return <DetailPlaceholder text={orgs.placeholder} />
  return <OrgView key={orgs.detail.id} org={orgs.detail} />
}

const linkClass =
  'inline-flex h-7 items-center gap-1.5 rounded-[7px] px-2 text-[13px] text-ink underline decoration-line-strong underline-offset-[3px] transition-colors duration-100 hover:bg-hover hover:decoration-ink [&_svg]:size-4 [&_svg]:text-ink-2'

function OrgView({ org }: { org: OrgDetailView }) {
  const [, send] = useModel('orgs')
  const [, sendShell] = useModel('shell')
  const where = { orgId: org.id, orgName: org.name }
  return (
    <div className="kit-scroll min-h-0 flex-1 overflow-y-auto">
      <article aria-label={org.name} className="flex max-w-[960px] flex-col gap-4 px-6 pb-8 pt-5">
        <header data-tauri-drag-region>
          <p className="text-[12px] text-ink-2">{org.headerLine}</p>
          <h2 className="mt-1 select-text text-[22px] font-semibold leading-[1.25] tracking-[-0.01em] text-ink">{org.name}</h2>
        </header>

        <nav aria-label="Related" className="-ml-2 flex flex-wrap items-center gap-x-2 gap-y-1">
          <button type="button" className={linkClass} onClick={() => void sendShell({ type: 'openOrgPeople', ...where })}>
            <PeopleIcon aria-hidden />
            {org.peopleLabel}
          </button>
          <button type="button" className={linkClass} onClick={() => void sendShell({ type: 'openOrgConfigItems', ...where })}>
            <ServerIcon aria-hidden />
            {org.configItemsLabel}
          </button>
          <Tooltip content={`New ticket for ${org.name}`} describe>
            <button type="button" className={linkClass} onClick={() => void sendShell({ type: 'newTicketForOrg', ...where })}>
              <NewTicketIcon aria-hidden />
              New ticket
            </button>
          </Tooltip>
        </nav>

        <section aria-labelledby="org-tickets" className="mt-1 flex flex-col gap-3">
          <div className="flex items-start gap-3">
            <div className="min-w-0 flex-1">
              <h3 id="org-tickets" className="text-[14px] font-semibold text-ink">
                {org.ticketsHeading}
              </h3>
              <ViewOptionsSummary className="mt-0.5" />
            </div>
            <ViewMenu />
          </div>
          <TableStateView
            state={org.tickets}
            label={`${org.name}: ${org.ticketsHeading}`}
            onOpen={(ticket) => void sendShell({ type: 'openTicket', ticket })}
            onPage={(page) => void send({ type: 'setTicketPage', page })}
            onRetry={() => void send({ type: 'retryTickets' })}
          />
        </section>
      </article>
    </div>
  )
}
