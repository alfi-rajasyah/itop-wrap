import { TicketTable, type TicketTableColumn, type TicketTableGroup } from '@/components/kit'
import type { TableState, TicketLink } from '@/backend'
import { InlineError, InlineLoading } from './paneStates'
import { SnapshotPager } from './SnapshotPager'

/**
 * An organization's or a person's tickets (ORG-10, ORG-11, PPL-13, PPL-14): loading, failed
 * (+ Retry when the snapshot allows it), empty, or the grouped table with its pager.
 */
export function TableStateView({
  state,
  label,
  onOpen,
  onPage,
  onRetry,
}: {
  state: TableState
  /** Accessible name of the table. */
  label: string
  onOpen: (ticket: TicketLink) => void
  onPage: (page: number) => void
  onRetry?: () => void
}) {
  switch (state.type) {
    case 'loading':
      return <InlineLoading>{state.text}</InlineLoading>
    case 'failed':
      return <InlineError onRetry={state.canRetry ? onRetry : undefined}>{state.text}</InlineError>
    case 'empty':
      return <p className="py-1 text-[13px] text-ink-2">{state.text}</p>
    case 'loaded': {
      const { table } = state
      const columns: TicketTableColumn[] = table.lastColumn === 'assignee' ? ['ref', 'title', 'caller', 'assignee'] : ['ref', 'title', 'caller']
      const refs = new Map<string, string>()
      const groups: TicketTableGroup[] = table.sections.map((s) => ({
        title: s.title,
        count: s.total,
        rows: s.rows.map((r) => {
          refs.set(r.ticketId, r.ref)
          return {
            id: r.ticketId,
            ticketRef: r.ref,
            title: r.title,
            priority: r.p1 ? 1 : undefined,
            needsYou: r.needsYou,
            caller: r.callerName,
            assigneeName: r.assignee.type === 'person' ? r.assignee.name : undefined,
            assigneeInitials: r.assignee.type === 'person' ? r.assignee.initials : undefined,
            unassigned: r.assignee.type === 'unassigned',
          }
        }),
      }))
      return (
        <div className="flex flex-col gap-3">
          <TicketTable columns={columns} groups={groups} label={label} onOpen={(id) => onOpen({ ticketId: id, ref: refs.get(id) ?? '' })} />
          <div className="flex justify-center">
            <SnapshotPager pager={table.pager} onPage={onPage} />
          </div>
        </div>
      )
    }
  }
}
