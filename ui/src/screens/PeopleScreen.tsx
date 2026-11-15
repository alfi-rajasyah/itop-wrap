import { useCallback, type ReactNode } from 'react'
import { InitialsAvatar, Listbox, MasterDetail, PaneHeader, SearchField, SimpleRow, Tabs, TabPanel } from '@/components/kit'
import { openUrl, useModel, type PeopleResults, type PeopleTab, type PersonDetailView } from '@/backend'
import { DetailPlaceholder, PaneLoading, PaneMessage } from '@/lib/paneStates'
import { ScopeBar } from '@/lib/ScopeBar'
import { TableStateView } from '@/lib/TableStateView'
import { useModelText } from '@/lib/useModelText'
import { ViewMenu, ViewOptionsSummary } from '@/lib/ViewMenu'

/* ─────────────────────────────────────────────────────────
 * PEOPLE (section 13): search by name (or an org's people) on the
 * left; the person on the right with their organization (a link to
 * Organizations), email (mailto), phone, teams, and the tickets
 * assigned to or reported by them under the shared View menu.
 * ───────────────────────────────────────────────────────── */

export default function PeopleScreen(_props: object) {
  return <MasterDetail listLabel="Resize the people list" list={<PeopleList />} detail={<PeopleDetail />} />
}

function PeopleList() {
  const [people, send] = useModel('people')
  const [shell] = useModel('shell')
  const commit = useCallback((query: string) => send({ type: 'setQuery', query }), [send])
  const [query, setQuery] = useModelText(people?.query ?? '', commit)
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <PaneHeader title={shell?.contentTitle ?? 'People'}>
        <SearchField value={query} onChange={setQuery} placeholder="Search people by name" />
      </PaneHeader>
      {people?.scope ? <ScopeBar text={`People in ${people.scope.orgName}`} onClear={() => void send({ type: 'clearScope' })} /> : null}
      {people ? <Results results={people.results} selectedId={people.selectedId} /> : null}
    </div>
  )
}

function Results({ results, selectedId }: { results: PeopleResults; selectedId: string | null }) {
  const [, send] = useModel('people')
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
          {results.banner ? <p className="mx-3 mb-2 rounded-control bg-inset px-2.5 py-1.5 text-[12px] leading-snug text-ink-2 shadow-hairline">{results.banner}</p> : null}
          <Listbox label="People" selectedId={selectedId} onSelect={(id) => void send({ type: 'select', id })} className="gap-px px-1.5 pb-2">
            {results.people.map((p) => (
              <SimpleRow
                key={p.id}
                id={p.id}
                title={p.displayName}
                subtitle={p.subtitle}
                leading={<InitialsAvatar name={p.displayName} initials={p.initials} size={28} />}
              />
            ))}
          </Listbox>
        </div>
      )
  }
}

function PeopleDetail() {
  const [people] = useModel('people')
  if (!people) return <div className="flex-1" />
  if (!people.detail) return <DetailPlaceholder text={people.placeholder} />
  return <PersonView key={people.detail.id} person={people.detail} />
}

const linkClass = 'rounded-[3px] text-ink underline decoration-line-strong underline-offset-[3px] hover:decoration-ink'

function PersonView({ person }: { person: PersonDetailView }) {
  const [, send] = useModel('people')
  const [, sendShell] = useModel('shell')
  const rows: { label: string; value: ReactNode }[] = []
  if (person.organization) {
    const org = person.organization
    rows.push({
      label: 'Organization',
      value: (
        <button type="button" className={linkClass} onClick={() => void sendShell({ type: 'openOrg', orgId: org.orgId, orgName: org.orgName })}>
          {org.orgName}
        </button>
      ),
    })
  }
  if (person.email) {
    const email = person.email
    rows.push({
      label: 'Email',
      value: (
        <a
          href={`mailto:${email}`}
          className={linkClass}
          onClick={(e) => {
            e.preventDefault()
            void openUrl(`mailto:${email}`)
          }}
        >
          {email}
        </a>
      ),
    })
  }
  if (person.phone) rows.push({ label: 'Phone', value: <span className="select-text tabular-nums">{person.phone}</span> })
  if (person.teams) rows.push({ label: 'Teams', value: person.teams })

  const tabsId = `person-${person.id}`
  return (
    <div className="kit-scroll min-h-0 flex-1 overflow-y-auto">
      <article aria-label={person.displayName} className="flex max-w-[960px] flex-col gap-4 px-6 pb-8 pt-5">
        <header className="flex items-center gap-3">
          <InitialsAvatar name={person.displayName} initials={person.initials} size={44} />
          <div className="min-w-0">
            <h2 className="select-text text-[22px] font-semibold leading-[1.25] tracking-[-0.01em] text-ink">{person.displayName}</h2>
            <p className="text-[13px] text-ink-2">{person.kindLine}</p>
          </div>
        </header>

        {rows.length > 0 ? (
          <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-5 gap-y-2 rounded-card bg-inset px-4 py-3.5 text-[13px] shadow-hairline">
            {rows.map((r) => (
              <div key={r.label} className="contents">
                <dt className="text-ink-2">{r.label}</dt>
                <dd className="min-w-0 break-words text-ink">{r.value}</dd>
              </div>
            ))}
          </dl>
        ) : null}

        <section className="mt-1 flex flex-col gap-3">
          <div className="flex items-start gap-3">
            <div className="min-w-0 flex-1">
              <Tabs<PeopleTab>
                id={tabsId}
                label="Tickets"
                items={[
                  { value: 'assigned', label: person.assignedLabel },
                  { value: 'reported', label: person.reportedLabel },
                ]}
                value={person.tab}
                onChange={(tab) => void send({ type: 'setTab', tab })}
                className="-ml-2.5 flex-wrap"
              />
              <ViewOptionsSummary className="mt-1" />
            </div>
            <ViewMenu />
          </div>
          <TabPanel tabsId={tabsId} value={person.tab}>
            <TableStateView
              state={person.tickets}
              label={person.tab === 'assigned' ? person.assignedLabel : person.reportedLabel}
              onOpen={(ticket) => void sendShell({ type: 'openTicket', ticket })}
              onPage={(page) => void send({ type: 'setTicketPage', page })}
            />
          </TabPanel>
        </section>
      </article>
    </div>
  )
}
