/**
 * MOCK ONLY. Approximations of the Rust rules (ports of ITopKit) so the fixture looks right.
 * They are NOT the source of truth and must never be imported by screens.
 */
import type { Pager, TicketGroup, TicketSort } from '../contract'
import type { MockTicket } from './data'

export const GROUP_TITLES: Record<TicketGroup, string> = {
  escalatedTTR: 'Escalated TTR',
  escalatedTTO: 'Escalated TTO',
  new: 'New',
  assigned: 'Assigned',
  pending: 'Pending',
  other: 'Other',
  resolved: 'Resolved',
  closed: 'Closed',
}

const MINE_ORDER: TicketGroup[] = ['escalatedTTR', 'escalatedTTO', 'assigned', 'new', 'pending', 'other', 'resolved', 'closed']
const TEAM_ORDER: TicketGroup[] = ['escalatedTTR', 'escalatedTTO', 'new', 'assigned', 'pending', 'other', 'resolved', 'closed']

export function groupOf(status: string): TicketGroup {
  switch (status) {
    case 'escalated_ttr':
      return 'escalatedTTR'
    case 'escalated_tto':
      return 'escalatedTTO'
    case 'new':
    case 'assigned':
    case 'pending':
    case 'resolved':
    case 'closed':
      return status
    default:
      return 'other'
  }
}

export function statusLabel(status: string): string {
  if (status === 'escalated_ttr') return 'Escalated TTR'
  if (status === 'escalated_tto') return 'Escalated TTO'
  if (status === 'waiting_for_approval') return 'Waiting for approval'
  return status.slice(0, 1).toUpperCase() + status.slice(1).replaceAll('_', ' ')
}

export function ticketId(t: Pick<MockTicket, 'ticketClass' | 'key'>): string {
  return `${t.ticketClass}::${t.key}`
}

export function isUnassigned(t: MockTicket): boolean {
  return t.agentId === '0' || t.agentId === ''
}

export function needsYou(t: MockTicket, contactId: string, teamIds: string[]): boolean {
  if (t.status === 'escalated_ttr') return t.agentId === contactId
  if (t.status === 'escalated_tto') return isUnassigned(t) && teamIds.includes(t.teamId)
  return false
}

const FAR_PAST = -8.64e15
const FAR_FUTURE = 8.64e15

function cmp(a: (number | string)[], b: (number | string)[]): number {
  for (let i = 0; i < a.length; i++) {
    if (a[i]! < b[i]!) return -1
    if (a[i]! > b[i]!) return 1
  }
  return 0
}

export function sortTickets(list: MockTicket[], sort: TicketSort): MockTicket[] {
  const start = (t: MockTicket, missing: number) => t.startDate?.getTime() ?? missing
  const updated = (t: MockTicket) => t.lastUpdate?.getTime() ?? FAR_PAST
  return [...list].sort((a, b) => {
    switch (sort) {
      case 'priority':
        return cmp([a.priority, start(a, FAR_FUTURE), a.ref], [b.priority, start(b, FAR_FUTURE), b.ref])
      case 'lastUpdate':
        return cmp([updated(b), a.ref], [updated(a), b.ref])
      case 'newest':
        return cmp([start(b, FAR_PAST), a.ref], [start(a, FAR_PAST), b.ref])
      case 'oldest':
        return cmp([start(a, FAR_FUTURE), a.ref], [start(b, FAR_FUTURE), b.ref])
    }
  })
}

export interface RawSection {
  group: TicketGroup
  tickets: MockTicket[]
}

export function groups(tickets: MockTicket[], view: 'mine' | 'team', sort: TicketSort): RawSection[] {
  const order = view === 'mine' ? MINE_ORDER : TEAM_ORDER
  return order
    .map((group) => ({ group, tickets: sortTickets(tickets.filter((t) => groupOf(t.status) === group), sort) }))
    .filter((s) => s.tickets.length > 0)
}

export interface RawPage {
  sections: RawSection[]
  totals: Map<TicketGroup, number>
  pager: Pager
  page: number
}

export function paginate(all: RawSection[], size: number | null, requested: number): RawPage {
  const total = all.reduce((n, s) => n + s.tickets.length, 0)
  const totals = new Map(all.map((s) => [s.group, s.tickets.length] as const))
  const count: Pager = total === 0 ? { type: 'none' } : { type: 'count', text: total === 1 ? '1 ticket' : `${total} tickets` }
  if (size === null || size <= 0 || total === 0) return { sections: all, totals, pager: count, page: 1 }
  const pageCount = Math.max(1, Math.ceil(total / size))
  const page = Math.min(Math.max(1, requested), pageCount)
  const start = (page - 1) * size
  const end = Math.min(start + size, total)
  const sections: RawSection[] = []
  let index = 0
  for (const s of all) {
    const lo = Math.max(start, index)
    const hi = Math.min(end, index + s.tickets.length)
    if (lo < hi) sections.push({ group: s.group, tickets: s.tickets.slice(lo - index, hi - index) })
    index += s.tickets.length
  }
  const pager: Pager =
    pageCount > 1
      ? { type: 'pages', page, pageCount, label: `${start + 1}–${end} of ${total}`, canPrevious: page > 1, canNext: page < pageCount }
      : count
  return { sections, totals, pager, page }
}

export function searchFilter(tickets: MockTicket[], query: string): MockTicket[] {
  const words = query.toLowerCase().split(' ').filter(Boolean)
  if (words.length === 0) return tickets
  return tickets.filter((t) => {
    const hay = `${t.ref} ${t.title}`.toLowerCase()
    return words.every((w) => hay.includes(w))
  })
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' })
export const natural = (a: string, b: string) => collator.compare(a, b)

export function age(date: Date, now: Date): string {
  const s = Math.max(0, (now.getTime() - date.getTime()) / 1000)
  if (s < 60) return 'now'
  if (s < 3600) return `${Math.floor(s / 60)}m`
  if (s < 86400) return `${Math.floor(s / 3600)}h`
  return `${Math.floor(s / 86400)}d`
}

export function clock(seconds: number): string {
  const minutes = Math.floor(Math.max(0, seconds) / 60)
  return `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, '0')}`
}

export function total(seconds: number): string {
  const minutes = Math.floor(Math.max(0, seconds) / 60)
  const h = Math.floor(minutes / 60)
  const m = minutes % 60
  if (h === 0) return `${m}m`
  return m === 0 ? `${h}h` : `${h}h ${m}m`
}

export function counter(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds))
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(Math.floor(s / 3600))}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}`
}

const time24 = (d: Date) => `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`

export function logDate(date: Date, now: Date): string {
  const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  const days = Math.round((day(now) - day(date)) / 86_400_000)
  if (days === 0) return `Today ${time24(date)}`
  if (days === 1) return `Yesterday ${time24(date)}`
  if (days >= 2 && days < 7) return `${date.toLocaleDateString('en-GB', { weekday: 'short' })} ${time24(date)}`
  const dm = date.toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })
  return date.getFullYear() === now.getFullYear() ? `${dm} ${time24(date)}` : `${dm} ${date.getFullYear()}`
}

/** "16 Nov 2026 at 09:12" (abbreviated date, short time). */
export function abbreviated(date: Date): string {
  return `${date.toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })} at ${time24(date)}`
}

/** "since" time: today → time only, else abbreviated date and time. */
export function sinceText(date: Date, now: Date): string {
  const sameDay = date.toDateString() === now.toDateString()
  return sameDay ? time24(date) : abbreviated(date)
}

export function classLabel(name: string): string {
  const words: string[] = []
  let current = ''
  const chars = [...name]
  chars.forEach((c, i) => {
    const isUpper = c !== c.toLowerCase()
    const next = chars[i + 1]
    const nextIsLower = next !== undefined && next !== next.toUpperCase()
    const last = current.slice(-1)
    if (isUpper && current !== '' && ((last !== '' && last !== last.toUpperCase()) || nextIsLower)) {
      words.push(current)
      current = ''
    }
    current += c
  })
  if (current !== '') words.push(current)
  return words.map((w, i) => (i === 0 || w === w.toUpperCase() ? w : w.toLowerCase())).join(' ')
}

export function initialsOf(name: string): string {
  return name
    .split(' ')
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]!.toUpperCase())
    .join('')
}

export function withPeriod(reason: string): string {
  const r = reason.trim()
  return r === '' || '.!?'.includes(r.slice(-1)) ? r : `${r}.`
}

export function byteCount(bytes: number): string {
  if (bytes < 1000) return `${bytes} bytes`
  if (bytes < 1_000_000) return `${Math.round(bytes / 1000)} KB`
  return `${(bytes / 1_000_000).toFixed(1)} MB`
}

export function listJoin(names: string[]): string {
  return new Intl.ListFormat('en', { style: 'long', type: 'conjunction' }).format(names)
}

export function relativeNamed(date: Date, now: Date): string {
  const minutes = Math.round((now.getTime() - date.getTime()) / 60000)
  if (minutes < 1) return 'now'
  return new Intl.RelativeTimeFormat('en', { numeric: 'auto' }).format(-minutes, 'minute')
}
