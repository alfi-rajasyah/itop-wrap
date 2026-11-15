import type { ReactNode } from 'react'
import { useModel, type ListBody, type ListScope, type ListSnapshot, type Send } from '@/backend'
import {
  EmptyState,
  ErrorState,
  IconButton,
  ListFooter,
  ListSection,
  Listbox,
  LoadingState,
  MasterDetail,
  NoticeBanner,
  PaneHeader,
  RefreshIcon,
  SearchField,
  SearchIcon,
} from '@/components/kit'
import { isCommand, useWindowKeyDown } from '@/lib/keys'
import { useSyncedText } from '@/lib/useSyncedText'
import { ListControls } from './list/ListControls'
import { ListRow } from './list/ListRow'
import { SnapshotPager } from './list/SnapshotPager'
import { TimerNotice } from './list/TimerNotice'
import TicketDetail from './TicketDetail'
import TicketSheets from './TicketSheets'

/*
 * My tickets / Team tickets (W/TicketListView.swift, LIST-15…70): the list pane on the left
 * (title, Refresh, search, controls, status bars, the grouped rows, pager and "Updated …"), the
 * list's ticket detail on the right, and its sheets.
 */
export default function TicketListScreen({ scope }: { scope: ListScope }) {
  // Both list models have the same snapshot and intents; typing it as one keeps `send` simple.
  const [list, send] = useModel(`list:${scope}` as 'list:mine')
  const [shell] = useModel('shell')

  // ⌘R refreshes the shown list (LIST-64, KEY-3); App stops the web view reload.
  useWindowKeyDown((e) => {
    if (!isCommand(e, 'r')) return
    e.preventDefault()
    if (list && !list.refreshing) void send({ type: 'refresh' })
  })

  return (
    <MasterDetail
      listLabel="Resize the ticket list"
      list={list ? <ListPane list={list} send={send} title={shell?.contentTitle ?? ''} /> : <div className="flex-1 bg-app" />}
      detail={
        <>
          <TicketDetail scope={scope} />
          <TicketSheets scope={scope} />
        </>
      }
    />
  )
}

function ListPane({ list, send, title }: { list: ListSnapshot; send: Send<'list:mine'>; title: string }) {
  const [query, setQuery] = useSyncedText(list.query, (q) => send({ type: 'setQuery', query: q }))
  return (
    <div className="flex min-h-0 flex-1 flex-col bg-app">
      <PaneHeader
        title={title}
        actions={
          <IconButton
            label="Refresh"
            tooltip="Refresh (⌘R)"
            icon={<RefreshIcon />}
            disabled={list.refreshing}
            onClick={() => void send({ type: 'refresh' })}
            aria-keyshortcuts="Meta+R"
          />
        }
      >
        <SearchField value={query} onChange={setQuery} placeholder="R-000412 or VPN" label="Search tickets" />
      </PaneHeader>

      {list.showControls ? <ListControls orgFilter={list.orgFilter} send={send} /> : null}

      <div className="flex min-h-0 flex-1 flex-col border-t border-line">
        <Content list={list} send={send} />
      </div>

      <ListFooter
        pager={<SnapshotPager pager={list.pager} onPage={(page) => void send({ type: 'setPage', page })} />}
        refreshing={list.footerSpinner}
        updatedLabel={list.updatedText}
      />
    </div>
  )
}

function Content({ list, send }: { list: ListSnapshot; send: Send<'list:mine'> }) {
  const content = list.content
  switch (content.type) {
    case 'loading':
      return <LoadingState label="Loading tickets from iTop" />
    case 'failed':
      return <ErrorState message={content.message} onRetry={() => void send({ type: 'refresh' })} retrying={list.refreshing} />
    case 'message':
      return <EmptyState title={content.text} />
    case 'list':
      return <Body body={content.body} list={list} send={send} />
  }
}

/** The status bars above the rows, then the rows (LIST-24…27, LIST-37, LIST-62, LIST-69). */
function Body({ body, list, send }: { body: ListBody; list: ListSnapshot; send: Send<'list:mine'> }) {
  const [timer] = useModel('timer')
  const [, sendShell] = useModel('shell')
  const runningClock = timer?.running?.clock ?? null
  const assign = (ticketId: string) => () => void send({ type: 'rowAssignToMe', ticketId })

  const bars: ReactNode[] = []
  if (body.lookupBar) {
    const bar = body.lookupBar
    bars.push(
      <StatusLine key="lookup" tone={bar.type === 'failed' ? 'error' : 'quiet'} icon={bar.type === 'searching' ? <SearchIcon /> : null}>
        {bar.text}
      </StatusLine>,
    )
  }
  if (body.showNotices) {
    bars.push(<TimerNotice key="timer" />)
    if (body.notice) {
      bars.push(
        <NoticeBanner
          key="notice"
          tone={body.notice.isError ? 'error' : 'info'}
          onDismiss={() => void send({ type: 'clearNotice' })}
          dismissLabel="Dismiss"
        >
          {body.notice.text}
        </NoticeBanner>,
      )
    }
  }

  return (
    <>
      {bars.length ? <div className="flex shrink-0 flex-col gap-1.5 px-3 pt-2">{bars}</div> : null}
      <div className="kit-scroll min-h-0 flex-1 overflow-y-auto pb-2">
        {body.orgNoMatch ? (
          <div className="flex flex-col items-start gap-1 px-4 pb-1 pt-3 text-[13px] text-ink-2">
            <p>{body.orgNoMatch.text}</p>
            {body.orgNoMatch.seeAll ? (
              <LinkButton
                onClick={() => {
                  const org = body.orgNoMatch!.seeAll!
                  void sendShell({ type: 'openOrg', orgId: org.orgId, orgName: org.orgName })
                }}
              >
                See all of its tickets
              </LinkButton>
            ) : null}
          </div>
        ) : null}
        {body.refreshError ? <p className="px-4 pb-1 pt-3 text-[12px] leading-snug text-danger">{body.refreshError}</p> : null}
        {body.noMatchText ? <p className="px-4 pb-1 pt-3 text-[13px] text-ink-2">{body.noMatchText}</p> : null}

        <Listbox
          label={list.scope === 'mine' ? 'My tickets' : 'Team tickets'}
          selectedId={list.selection}
          onSelect={(ticketId) => void send({ type: 'select', ticketId })}
          className="pt-1"
        >
          {body.extraSection ? (
            <ListSection title={body.extraSection.title}>
              <ListRow row={body.extraSection.row} runningClock={runningClock} onAssignToMe={assign(body.extraSection.row.ticket.id)} />
            </ListSection>
          ) : null}
          {body.sections.map((section) => (
            <ListSection key={section.group} title={section.title} count={section.total}>
              {section.rows.map((row) => (
                <ListRow key={row.ticket.id} row={row} runningClock={runningClock} onAssignToMe={assign(row.ticket.id)} />
              ))}
            </ListSection>
          ))}
        </Listbox>
      </div>
    </>
  )
}

/** A one-line status above the list: the ref lookup (LIST-24…26). */
function StatusLine({ tone, icon, children }: { tone: 'quiet' | 'error'; icon: ReactNode; children: ReactNode }) {
  return (
    <p
      role={tone === 'error' ? 'alert' : 'status'}
      className={`flex min-h-8 items-center gap-2 px-1 text-[12.5px] leading-[18px] ${tone === 'error' ? 'text-danger' : 'text-ink-2'} [&_svg]:size-3.5 [&_svg]:shrink-0`}
    >
      {icon}
      <span className="min-w-0">{children}</span>
    </p>
  )
}

function LinkButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" onClick={onClick} className="rounded-[4px] text-[13px] font-medium text-ink underline underline-offset-2 hover:decoration-2">
      {children}
    </button>
  )
}
