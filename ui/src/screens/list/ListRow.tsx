import type { ReactNode } from 'react'
import type { RowAgent, TicketRow as Row } from '@/backend'
import { Assignee, AssignToMePill, TicketRow } from '@/components/kit'

/**
 * One ticket in My/Team tickets (LIST-52…58). Line 1: needs-you dot, ref, priority mark, age.
 * Line 2: title. Line 3: caller, then the running clock or the time spent, then the agent.
 * Everything comes from the row snapshot; the live clock from `timer.running.clock`.
 */
export function ListRow({ row, runningClock, onAssignToMe }: { row: Row; runningClock: string | null; onAssignToMe: () => void }) {
  const t = row.ticket
  return (
    <TicketRow
      id={t.id}
      ticketRef={t.ref}
      title={t.title}
      priority={row.priorityMark === 'p1' ? 1 : row.priorityMark === 'p2' ? 2 : 4}
      needsYou={t.needsYou}
      age={row.age ?? undefined}
      ageTitle={row.openedTooltip ?? undefined}
      caller={t.callerName || undefined}
      runningLabel={row.time.type === 'running' ? runningClock : null}
      total={row.time.type === 'spent' ? row.time.text : undefined}
      assignee={agentOf(row.agent, t.ref, onAssignToMe)}
    />
  )
}

function agentOf(agent: RowAgent, ref: string, onAssignToMe: () => void): ReactNode {
  switch (agent.type) {
    case 'hidden':
      return undefined
    case 'assignToMe':
      return <AssignToMePill ticketRef={ref} onClick={onAssignToMe} />
    case 'unassigned':
      return <Assignee unassigned />
    case 'you':
      return <span className="truncate text-[12px] text-ink-2">You</span>
    case 'name':
      return <span className="truncate text-[12px] text-ink-2">{agent.name}</span>
  }
}
