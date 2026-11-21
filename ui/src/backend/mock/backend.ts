/**
 * MOCK BACKEND: an in-memory stand-in for the Rust models, used when the UI runs in a plain
 * browser (no Tauri) and in UI tests. It is a UI FIXTURE, NOT THE SOURCE OF TRUTH: rules are
 * implemented approximately so every screen and state can be reached by clicking. The real rules
 * live in Rust (crates/itop-kit, src-tauri) and are checked against docs/rewrite/parity-checklist.md.
 *
 * Demo data is the Swift app's ITOPWRAP_DEMO set (Sources/ITopWrap/App.swift). Demo triggers, as in
 * the Swift demo: "fail" in a log entry / pending reason / solution / new ticket title fails it;
 * "refuse" in an entry with images fails the upload; R-000436 plays the "colleague took it" race;
 * R-000405 has a running web stopwatch; R-000512 is the not-in-queue lookup. Sign-in passwords:
 * "wrong" = invalid credentials, "rest" = REST profile refused, "offline" = can't reach; anything
 * else signs in.
 */
import type {
  AgentChip,
  AppError,
  Attachments,
  CIChip,
  Composer,
  DetailSnapshot,
  Dropdown,
  ImageIntakeItem,
  IntentOf,
  ListContent,
  ListScope,
  LoginMessage,
  LogTab,
  MenuRow,
  ModelId,
  Notice,
  PeopleTab,
  PickItem,
  PrimaryAction,
  RangeDays,
  ResultOf,
  RichContent,
  SidebarItem,
  SnapshotOf,
  TableState,
  Ticket,
  TicketClass,
  TicketRow,
  TicketSection,
  TicketSort,
  TimerButton,
  TimerNotice,
  UpdateDialog,
} from '../contract'
import { BackendError, type Backend } from '../transport'
import {
  AGENTS,
  ATTACHMENTS,
  BASE_TOTALS,
  CALLER_OF,
  CALLERS,
  demoCMDB,
  demoDetails,
  demoTickets,
  emptyDetail,
  HOST,
  IMPACTS,
  ME,
  NEW_TICKET_ORGS,
  ORG_COUNTS,
  ORGS,
  PEOPLE,
  RACE_REF,
  type MockCI,
  type MockCIDetail,
  type MockDetail,
  type MockPerson,
  type MockTicket,
  type Ref,
  RESOLUTION_CODES,
  SERVER,
  SERVICES,
  SUBCATEGORIES,
  TEAMS,
  URGENCIES,
  WEB_STOPWATCH_REF,
} from './data'
import {
  abbreviated,
  age,
  byteCount,
  classLabel,
  clock,
  counter,
  GROUP_TITLES,
  groups,
  initialsOf,
  isUnassigned,
  listJoin,
  logDate,
  natural,
  needsYou,
  paginate,
  relativeNamed,
  searchFilter,
  sinceText,
  statusLabel,
  ticketId,
  total,
  withPeriod,
} from './rules'

export interface MockOptions {
  /** Simulated iTop latency. 0 in tests. */
  delayMs: number
  /** Forced list state, like ITOPWRAP_DEMO_STATE. */
  listState: 'normal' | 'empty' | 'loading' | 'error'
  /** Start on the sign-in form. */
  signedOut: boolean
  /** Start on the "Can't reach iTop" screen. */
  unreachable: boolean
  /** Saving time fails, like ITOPWRAP_DEMO_TIMEFAIL. */
  timeFail: boolean
  /** 60 bulk tickets in Team tickets, like ITOPWRAP_DEMO_MANY. */
  many: boolean
  /** Writes switched off, like a debug build without a write allowlist. */
  readOnly: boolean
  /** The clock; tests pass their own to move time forward. */
  now: () => Date
}

export const DEFAULT_MOCK_OPTIONS: MockOptions = {
  delayMs: 300,
  listState: 'normal',
  signedOut: false,
  unreachable: false,
  timeFail: false,
  many: false,
  readOnly: false,
  now: () => new Date(),
}

const WRITES_OFF = 'Changes to iTop are switched off in this build'
const NETWORK_ERROR = `Can't reach ${HOST}. Check your connection, then retry.`
const SERVER_ERROR = (message: string) => `iTop returned error 100: ${message}`
const MAX_IMAGES = 5
const MAX_IMAGE_BYTES = 4 * 1_048_576
const MAX_INTAKE_BYTES = 40 * 1_048_576
const TIMER_LIMIT = 4 * 3600

type Loadable<T> = { type: 'idle' } | { type: 'loading' } | { type: 'loaded'; value: T } | { type: 'failed'; message: string }
type Lookup =
  | { type: 'none' }
  | { type: 'searching'; ref: string }
  | { type: 'found'; ticket: MockTicket }
  | { type: 'missing'; ref: string }
  | { type: 'failed'; message: string }

interface MockImage {
  id: string
  filename: string
  size: number
  dataUrl: string
}

interface ListState {
  scope: ListScope
  phase: { type: 'loading' } | { type: 'loaded' } | { type: 'failed'; message: string }
  tickets: MockTicket[]
  updatedAt: Date | null
  refreshing: boolean
  refreshAgain: boolean
  refreshError: string | null
  lookup: Lookup
  justChanged: string | null
  query: string
  queryToken: number
  selection: string | null
  orgFilter: string | null
  page: number
  working: string | null
  notice: Notice | null
  drafts: Record<string, string>
  draftImages: Record<string, MockImage[]>
}

interface DetailState {
  ticketId: string
  fallback: MockTicket
  signature: string
  phase: { type: 'loading' } | { type: 'loaded'; detail: MockDetail } | { type: 'failed'; message: string }
  tab: LogTab
  copied: boolean
  sending: boolean
  sendError: string | null
  attachments: Attachments
  loadToken: number
}

interface AssignState {
  ticketId: string
  teams: Loadable<Ref[]>
  agents: Loadable<Ref[]>
  teamId: string | null
  agentId: string | null
  error: string | null
}

interface PendingState {
  ticketId: string
  reason: string
  tried: boolean
  error: string | null
}

interface ResolveState {
  ticketId: string
  services: Loadable<Ref[]>
  subcategories: Loadable<Ref[]>
  serviceId: string | null
  subcategoryId: string | null
  codeId: string | null
  solution: string
  tried: boolean
  error: string | null
}

interface NewTicketDraft {
  ticketClass: TicketClass
  orgId: string | null
  callerId: string | null
  serviceId: string | null
  subcategoryId: string | null
  impact: string | null
  urgency: string | null
  title: string
  description: string
}

interface NewTicketState extends NewTicketDraft {
  start: { orgId: string | null; orgName: string | null; ciId: string | null; ciName: string | null }
  orgs: Loadable<Ref[]>
  callers: Loadable<Ref[]>
  services: Loadable<Ref[]>
  subcategories: Loadable<Ref[]>
  tried: boolean
  creating: boolean
  error: string | null
}

interface TimerState {
  running: { ticketId: string; ref: string; title: string; start: Date } | null
  saving: boolean
  notice: TimerNotice | null
  failed: { seconds: number; ref: string; ticketId: string } | null
  totals: Record<string, number>
  web: Record<string, Date>
}

/** `resolve:<scope>` sheets plus the menu bar's Resolve window. */
type ResolveSlot = ListScope | 'window'

const MOCK_CURRENT = '2.0.0'
const MOCK_LATEST = '2.1.0'

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms))

const isID = (s: string | null | undefined): s is string => !!s && /^\d+$/.test(s) && s !== '0'

function base64Bytes(b64: string): Uint8Array {
  const binary = atob(b64)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
  return bytes
}

function sniffImage(bytes: Uint8Array): string | null {
  const starts = (sig: number[]) => sig.every((b, i) => bytes[i] === b)
  if (starts([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) return 'image/png'
  if (starts([0xff, 0xd8, 0xff])) return 'image/jpeg'
  if (starts([0x47, 0x49, 0x46, 0x38, 0x37, 0x61]) || starts([0x47, 0x49, 0x46, 0x38, 0x39, 0x61])) return 'image/gif'
  return null
}

const ACTIONS: Record<string, ('assign' | 'reassign' | 'pending' | 'resolve' | 'reopen' | 'close')[]> = {
  new: ['assign'],
  escalated_tto: ['assign'],
  assigned: ['reassign', 'pending', 'resolve'],
  escalated_ttr: ['reassign', 'pending', 'resolve'],
  pending: ['reassign', 'resolve'],
  resolved: ['reopen', 'close'],
}
const actionsFor = (status: string) => ACTIONS[status] ?? []

function primaryFor(status: string): 'assign' | 'resolve' | 'close' | null {
  const actions = actionsFor(status)
  for (const candidate of ['assign', 'resolve', 'close'] as const) if (actions.includes(candidate)) return candidate
  return null
}

const SIDEBAR: { item: SidebarItem; title: string }[] = [
  { item: 'myTickets', title: 'My tickets' },
  { item: 'teamTickets', title: 'Team tickets' },
  { item: 'configItems', title: 'Config items' },
  { item: 'organizations', title: 'Organizations' },
  { item: 'people', title: 'People' },
]

const RANGE_TITLES: Record<RangeDays, string> = { 0: 'Any time', 7: 'Last 7 days', 30: 'Last 30 days', 90: 'Last 90 days' }
const SORT_TITLES: Record<TicketSort, string> = {
  priority: 'Priority, then oldest',
  lastUpdate: 'Last update, newest first',
  newest: 'Opened, newest first',
  oldest: 'Opened, oldest first',
}
const PAGE_SIZES: (number | null)[] = [10, 20, 30, 50, 100, null]

function newList(scope: ListScope): ListState {
  return {
    scope, phase: { type: 'loading' }, tickets: [], updatedAt: null, refreshing: false, refreshAgain: false, refreshError: null,
    lookup: { type: 'none' }, justChanged: null, query: '', queryToken: 0, selection: null, orgFilter: null, page: 1, working: null,
    notice: null, drafts: {}, draftImages: {},
  }
}

function blankDraft(ticketClass: TicketClass = 'UserRequest'): NewTicketDraft {
  return { ticketClass, orgId: null, callerId: null, serviceId: null, subcategoryId: null, impact: '1', urgency: '4', title: '', description: '' }
}

function placeholderOf<T>(state: Loadable<T[]>, loading: string, empty: string): string {
  switch (state.type) {
    case 'idle':
    case 'loading':
      return loading
    case 'failed':
      return `Couldn't load: ${state.message}`
    case 'loaded':
      return state.value.length === 0 ? empty : 'Choose…'
  }
}

const valueOf = <T>(state: Loadable<T>): T | null => (state.type === 'loaded' ? state.value : null)
const pick = (refs: Ref[]): PickItem[] => refs.map((r) => ({ id: r.id, label: r.name }))

export class MockBackend implements Backend {
  readonly kind = 'mock' as const
  readonly options: MockOptions
  private readonly subscribers = new Map<ModelId, Set<(snapshot: unknown) => void>>()
  private readonly emitted = new Map<ModelId, string>()
  private tickTimer: ReturnType<typeof setInterval> | null = null
  private clockTimer: ReturnType<typeof setInterval> | null = null
  private disposed = false

  // Session
  private phase: 'restoring' | 'signedOut' | 'unreachable' | 'signedIn'
  private unreachableMessage = "Can't reach the server. Check your connection and the server address."
  private server = SERVER
  private user = ME.login
  private signingIn = false
  private loginMessage: LoginMessage | null = null
  private loginNotice: string | null = null

  // Workspace
  private selection: SidebarItem = 'myTickets'
  private range: RangeDays = 0
  private includeClosed = false
  private sort: TicketSort = 'priority'
  private pageSize: number | null = 20
  private readonly db = new Map<string, MockTicket>()
  private readonly details: Record<string, MockDetail>
  private readonly added: Record<string, { text: string; images: string[]; isPrivate: boolean; date: Date }[]> = {}
  private readonly lists: Record<ListScope, ListState> = { mine: newList('mine'), team: newList('team') }
  private readonly detail: Record<ListScope, DetailState | null> = { mine: null, team: null }
  private readonly assign: Record<ListScope, AssignState | null> = { mine: null, team: null }
  private readonly pending: Record<ListScope, PendingState | null> = { mine: null, team: null }
  /** The two list sheets and the menu bar's Resolve window (which works on My tickets). */
  private readonly resolve: Record<ResolveSlot, ResolveState | null> = { mine: null, team: null, window: null }
  private readonly newTicket: Record<'sheet' | 'window' | 'edit', NewTicketState | null> = { sheet: null, window: null, edit: null }
  /** Edit ticket: the ticket and its fields as opened (to tell what changed). */
  private editTarget: { ticket: MockTicket; original: NewTicketDraft; locked: boolean } | null = null
  private newTicketDraft: NewTicketDraft | null = null
  private assignToMePref = true
  private lastTicketNumber = 600
  private readonly webSince: Date
  private readonly timer: TimerState = { running: null, saving: false, notice: null, failed: null, totals: {}, web: {} }
  private menuQuery = ''
  private readonly cmdbData: { items: MockCI[]; details: Record<string, MockCIDetail> }
  private readonly cmdb = {
    query: '',
    token: 0,
    scope: null as { orgId: string; orgName: string } | null,
    results: { type: 'idle' } as Loadable<MockCI[]>,
    searched: null as string | null,
    selectedId: null as string | null,
    current: null as MockCI | null,
    detail: { type: 'loading' } as Loadable<MockCIDetail>,
    copied: false,
  }
  private readonly orgs = {
    query: '',
    list: { type: 'idle' } as Loadable<{ id: string; name: string; code: string; openCount: number }[]>,
    selectedId: null as string | null,
    tickets: { type: 'idle' } as Loadable<MockTicket[]>,
    counts: null as { people: number; configItems: number } | null,
    page: 1,
  }
  private readonly people = {
    query: '',
    token: 0,
    scope: null as { orgId: string; orgName: string } | null,
    results: { type: 'idle' } as Loadable<MockPerson[]>,
    total: 0,
    searched: null as string | null,
    selectedId: null as string | null,
    tab: 'assigned' as PeopleTab,
    assigned: { type: 'idle' } as Loadable<MockTicket[]>,
    reported: { type: 'idle' } as Loadable<MockTicket[]>,
    page: 1,
  }

  readonly native = {
    openUrl: async (url: string) => {
      if (!/^(https?|mailto):/i.test(url)) throw new BackendError({ type: 'invalidIntent', message: `Refused to open ${url}` })
      if (typeof window !== 'undefined' && typeof window.open === 'function') window.open(url, '_blank', 'noopener')
    },
  }

  constructor(options: Partial<MockOptions> = {}) {
    this.options = { ...DEFAULT_MOCK_OPTIONS, ...options }
    const now = this.now()
    const { mine, others, found, closed, bulk } = demoTickets(now, this.options.many)
    for (const t of [...mine, ...others, found, ...closed, ...bulk]) this.db.set(t.ref, t)
    this.details = demoDetails(now)
    this.cmdbData = demoCMDB(now)
    this.webSince = new Date(now.getTime() - 25 * 60_000)
    this.timer.totals = { ...BASE_TOTALS }
    this.phase = this.options.unreachable ? 'unreachable' : this.options.signedOut ? 'signedOut' : 'signedIn'
    if (this.phase === 'signedIn') void this.startWorkspace()
    this.clockTimer = setInterval(() => this.changed(), 30_000)
    ;(this.clockTimer as { unref?: () => void }).unref?.()
  }

  /** Stops the timers (tests). */
  dispose() {
    this.disposed = true
    if (this.tickTimer) clearInterval(this.tickTimer)
    if (this.clockTimer) clearInterval(this.clockTimer)
    this.subscribers.clear()
  }

  private now() {
    return this.options.now()
  }

  private wait(ms = this.options.delayMs) {
    return sleep(ms === this.options.delayMs ? ms : Math.round((ms * this.options.delayMs) / 300))
  }

  private get canWrite() {
    return !this.options.readOnly
  }

  // ===========================================================================================
  // Transport
  // ===========================================================================================

  async getSnapshot<K extends ModelId>(model: K): Promise<SnapshotOf<K> | null> {
    return this.build(model)
  }

  subscribe<K extends ModelId>(model: K, callback: (snapshot: SnapshotOf<K> | null) => void): () => void {
    const set = this.subscribers.get(model) ?? new Set()
    const wrapped = callback as (snapshot: unknown) => void
    set.add(wrapped)
    this.subscribers.set(model, set)
    return () => {
      set.delete(wrapped)
    }
  }

  async dispatch<K extends ModelId, I extends IntentOf<K>>(model: K, intent: I): Promise<ResultOf<K, I['type']>> {
    if (model !== 'session' && model !== 'settings' && this.phase !== 'signedIn') {
      throw new BackendError({ type: 'notSignedIn', message: `${model} needs a signed-in session` } satisfies AppError)
    }
    try {
      const result = await this.handle(model, intent)
      return (result ?? null) as ResultOf<K, I['type']>
    } finally {
      this.changed()
    }
  }

  /** Recomputes every watched snapshot and pushes the ones that changed. */
  private changed() {
    if (this.disposed) return
    for (const [model, set] of this.subscribers) {
      if (set.size === 0) continue
      const snapshot = this.build(model)
      const json = JSON.stringify(snapshot)
      if (this.emitted.get(model) === json) continue
      this.emitted.set(model, json)
      set.forEach((callback) => callback(snapshot))
    }
  }

  private build<K extends ModelId>(model: K): SnapshotOf<K> | null {
    const builders: { [M in ModelId]: () => SnapshotOf<M> | null } = {
      session: () => this.sessionSnapshot(),
      shell: () => this.ws(() => this.shellSnapshot()),
      options: () => this.ws(() => this.optionsSnapshot()),
      'list:mine': () => this.ws(() => this.listSnapshot('mine')),
      'list:team': () => this.ws(() => this.listSnapshot('team')),
      'detail:mine': () => this.ws(() => this.detailSnapshot('mine')),
      'detail:team': () => this.ws(() => this.detailSnapshot('team')),
      'assign:mine': () => this.ws(() => this.assignSnapshot('mine')),
      'assign:team': () => this.ws(() => this.assignSnapshot('team')),
      'pending:mine': () => this.ws(() => this.pendingSnapshot('mine')),
      'pending:team': () => this.ws(() => this.pendingSnapshot('team')),
      'resolve:mine': () => this.ws(() => this.resolveSnapshot('mine')),
      'resolve:team': () => this.ws(() => this.resolveSnapshot('team')),
      'resolve:window': () => this.ws(() => this.resolveSnapshot('window')),
      'newTicket:sheet': () => this.ws(() => this.newTicketSnapshot('sheet')),
      'newTicket:window': () => this.ws(() => this.newTicketSnapshot('window')),
      editTicket: () => this.ws(() => this.newTicketSnapshot('edit')),
      timer: () => this.ws(() => this.timerSnapshot()),
      cmdb: () => this.ws(() => this.cmdbSnapshot()),
      orgs: () => this.ws(() => this.orgsSnapshot()),
      people: () => this.ws(() => this.peopleSnapshot()),
      menubar: () => this.ws(() => this.menuSnapshot()),
      update: () => this.ws(() => this.updateSnapshot()),
      settings: () => this.settingsSnapshot(),
    }
    return builders[model]() as SnapshotOf<K> | null
  }

  private ws<T>(make: () => T): T | null {
    return this.phase === 'signedIn' ? make() : null
  }

  private async handle(model: ModelId, intent: { type: string } & Record<string, unknown>): Promise<unknown> {
    const scopeOf = (id: string) => id.split(':')[1] as ListScope
    switch (model) {
      case 'session':
        return this.sessionIntent(intent as IntentOf<'session'>)
      case 'shell':
        return this.shellIntent(intent as IntentOf<'shell'>)
      case 'options':
        return this.optionsIntent(intent as IntentOf<'options'>)
      case 'list:mine':
      case 'list:team':
        return this.listIntent(scopeOf(model), intent as IntentOf<'list:mine'>)
      case 'detail:mine':
      case 'detail:team':
        return this.detailIntent(scopeOf(model), intent as IntentOf<'detail:mine'>)
      case 'assign:mine':
      case 'assign:team':
        return this.assignIntent(scopeOf(model), intent as IntentOf<'assign:mine'>)
      case 'pending:mine':
      case 'pending:team':
        return this.pendingIntent(scopeOf(model), intent as IntentOf<'pending:mine'>)
      case 'resolve:mine':
      case 'resolve:team':
        return this.resolveIntent(scopeOf(model), intent as IntentOf<'resolve:mine'>)
      case 'resolve:window':
        return this.resolveIntent('window', intent as IntentOf<'resolve:mine'>)
      case 'newTicket:sheet':
      case 'newTicket:window':
        return this.newTicketIntent(model === 'newTicket:sheet' ? 'sheet' : 'window', intent as IntentOf<'newTicket:sheet'>)
      case 'editTicket':
        return this.newTicketIntent('edit', intent as IntentOf<'newTicket:sheet'>)
      case 'timer':
        return this.timerIntent(intent as IntentOf<'timer'>)
      case 'cmdb':
        return this.cmdbIntent(intent as IntentOf<'cmdb'>)
      case 'orgs':
        return this.orgsIntent(intent as IntentOf<'orgs'>)
      case 'people':
        return this.peopleIntent(intent as IntentOf<'people'>)
      case 'menubar':
        return this.menuIntent(intent as IntentOf<'menubar'>)
      case 'update':
        return this.updateIntent(intent as IntentOf<'update'>)
      case 'settings':
        return this.settingsIntent(intent as IntentOf<'settings'>)
    }
  }

  // ===========================================================================================
  // Shared views
  // ===========================================================================================

  private ticketView(t: MockTicket): Ticket {
    return {
      id: ticketId(t), key: t.key, ticketClass: t.ticketClass, ref: t.ref, title: t.title, status: t.status, statusLabel: statusLabel(t.status),
      priority: t.priority, callerName: t.callerName, orgName: t.orgName, orgId: t.orgId, teamId: t.teamId, teamName: t.teamName,
      agentId: t.agentId, agentName: t.agentName, startDate: t.startDate?.toISOString() ?? null, lastUpdate: t.lastUpdate?.toISOString() ?? null,
      isUnassigned: isUnassigned(t), isMine: t.agentId === ME.contactId, needsYou: this.needsYou(t),
    }
  }

  private needsYou(t: MockTicket) {
    return needsYou(t, ME.contactId, ME.teams.map((x) => x.id))
  }

  private canAssign(t: MockTicket) {
    return ['new', 'escalated_tto', 'assigned', 'escalated_ttr', 'pending'].includes(t.status)
  }

  private row(t: MockTicket, scope: ListScope, extra: boolean): TicketRow {
    const id = ticketId(t)
    const showAgent = scope === 'team' || isUnassigned(t) || extra
    let agent: TicketRow['agent'] = { type: 'hidden' }
    if (showAgent) {
      if (isUnassigned(t) && this.canAssign(t) && this.canWrite) agent = { type: 'assignToMe' }
      else if (isUnassigned(t)) agent = { type: 'unassigned' }
      else if (t.agentId === ME.contactId) agent = { type: 'you' }
      else agent = { type: 'name', name: t.agentName }
    }
    let time: TicketRow['time'] = { type: 'none' }
    if (!extra) {
      const spent = this.timer.totals[id] ?? 0
      if (this.timer.running?.ticketId === id) time = { type: 'running' }
      else if (spent > 0) time = { type: 'spent', text: total(spent) }
    }
    return {
      ticket: this.ticketView(t),
      age: t.startDate ? age(t.startDate, this.now()) : null,
      openedTooltip: t.startDate ? `Opened ${abbreviated(t.startDate)}` : null,
      priorityMark: t.priority === 1 ? 'p1' : t.priority === 2 ? 'p2' : null,
      agent,
      time,
    }
  }

  private agentChip(t: MockTicket): AgentChip {
    if (isUnassigned(t)) return { type: 'unassigned' }
    if (t.agentId === ME.contactId) return { type: 'person', initials: ME.initials, name: 'You' }
    return { type: 'person', initials: initialsOf(t.agentName), name: t.agentName }
  }

  private table(tickets: MockTicket[], lastColumn: 'assignee' | 'caller', page: number): TableState {
    const paged = paginate(groups(tickets, 'team', this.sort), this.pageSize, page)
    return {
      type: 'loaded',
      table: {
        lastColumn,
        sections: paged.sections.map((s) => ({
          group: s.group, title: GROUP_TITLES[s.group], total: paged.totals.get(s.group) ?? 0,
          rows: s.tickets.map((t) => ({
            ticketId: ticketId(t), ref: t.ref, title: t.title, needsYou: this.needsYou(t), p1: t.priority === 1,
            callerName: t.callerName, assignee: this.agentChip(t),
          })),
        })),
        pager: paged.pager,
      },
    }
  }

  private isDefaultScope() {
    return this.range === 0 && !this.includeClosed
  }

  private webUrl(cls: string, key: string) {
    return `${this.server}/pages/UI.php?operation=details&class=${cls}&id=${key}`
  }

  private findTicket(id: string): MockTicket | null {
    for (const t of this.db.values()) if (ticketId(t) === id) return t
    return null
  }

  // ===========================================================================================
  // session
  // ===========================================================================================

  private sessionSnapshot(): SnapshotOf<'session'> {
    switch (this.phase) {
      case 'restoring':
        return { type: 'restoring' }
      case 'unreachable':
        return { type: 'unreachable', message: this.unreachableMessage }
      case 'signedIn':
        return { type: 'signedIn', displayName: ME.displayName, host: HOST }
      case 'signedOut': {
        const message = this.loginMessage ?? (this.loginNotice ? { type: 'notice' as const, text: this.loginNotice } : null)
        return {
          type: 'signedOut',
          form: {
            server: this.server, user: this.user, httpWarning: this.server.trim().toLowerCase().startsWith('http://'),
            signingIn: this.signingIn, message,
            initialFocus: this.server.length <= 'https://'.length ? 'server' : this.user === '' ? 'user' : 'password',
          },
        }
      }
    }
  }

  private async sessionIntent(intent: IntentOf<'session'>): Promise<unknown> {
    switch (intent.type) {
      case 'setServer':
        this.server = intent.server
        return null
      case 'setUser':
        this.user = intent.user
        return null
      case 'signIn': {
        if (this.signingIn) return { ok: false, focus: null }
        this.loginMessage = null
        if (this.user.trim() === '' || intent.password === '') {
          this.loginMessage = { type: 'missingFields', text: 'Enter your username and password.' }
          return { ok: false, focus: this.user.trim() === '' ? 'user' : 'password' }
        }
        this.signingIn = true
        this.changed()
        await this.wait()
        this.signingIn = false
        let server = this.server.trim()
        if (server === '') {
          this.loginMessage = { type: 'error', text: "That server address can't be used. Enter it like https://itop.example.com." }
          return { ok: false, focus: null }
        }
        if (intent.password === 'wrong') {
          this.loginMessage = { type: 'error', text: 'Invalid credentials. Check your username and password.' }
          return { ok: false, focus: 'password' }
        }
        if (intent.password === 'rest') {
          this.loginMessage = {
            type: 'restRefused', bold: "This account can't use the iTop REST API.",
            rest: ` Ask your iTop admin to add the "REST Services User" profile to ${this.user}, then sign in again.`,
          }
          return { ok: false, focus: null }
        }
        if (intent.password === 'offline') {
          this.loginMessage = { type: 'error', text: "Can't reach the server. Check your connection and the server address." }
          return { ok: false, focus: null }
        }
        if (!server.includes('://')) server = `https://${server}`
        server = server.replace(/\/(pages|webservices)\/.*$/i, '').replace(/[?#].*$/, '').replace(/\/+$/, '')
        this.server = server
        this.user = this.user.trim()
        this.loginNotice = null
        this.selection = 'myTickets'
        this.phase = 'signedIn'
        void this.startWorkspace()
        return { ok: true, focus: null }
      }
      case 'retryRestore':
        this.phase = 'restoring'
        this.changed()
        await this.wait()
        this.phase = 'signedIn'
        void this.startWorkspace()
        return null
      case 'signInWithAnotherAccount':
        this.phase = 'signedOut'
        this.loginNotice = null
        this.loginMessage = null
        return null
      case 'signOut':
        this.phase = 'signedOut'
        this.loginNotice = null
        this.loginMessage = null
        return null
    }
  }

  private async startWorkspace() {
    if (this.lists.mine.phase.type !== 'loaded') await this.refresh('mine')
  }

  // ===========================================================================================
  // shell
  // ===========================================================================================

  private shellSnapshot(): SnapshotOf<'shell'> {
    const needCount = this.needCount()
    return {
      selection: this.selection,
      sidebar: SIDEBAR.map((s) => ({ item: s.item, title: s.title, count: s.item === 'myTickets' && needCount > 0 ? needCount : null })),
      contentTitle: SIDEBAR.find((s) => s.item === this.selection)!.title,
      needCount,
      account: { initials: ME.initials, displayName: ME.displayName, host: HOST },
      signOutConfirm: { title: `Sign out of ${HOST}?`, message: 'Your username stays filled in.' },
      sidebarWidth: this.sidebarWidth,
      sidebarVisible: this.sidebarVisible,
    }
  }

  private sidebarWidth = 200
  private sidebarVisible = true

  private needCount() {
    return this.lists.mine.tickets.filter((t) => this.needsYou(t)).length
  }

  private async select(item: SidebarItem) {
    this.selection = item
    this.changed()
    if (item === 'myTickets' && this.lists.mine.phase.type !== 'loaded') await this.refresh('mine')
    if (item === 'teamTickets' && this.lists.team.phase.type !== 'loaded') await this.refresh('team')
    if (item === 'organizations') await this.loadOrgList()
  }

  private async shellIntent(intent: IntentOf<'shell'>): Promise<unknown> {
    switch (intent.type) {
      case 'select':
        await this.select(intent.item)
        return null
      case 'openTicket': {
        const { ticketId: id, ref } = intent.ticket
        if (this.lists.mine.tickets.some((t) => ticketId(t) === id)) {
          this.setQuery('mine', '')
          this.lists.mine.selection = id
          this.syncDetails()
          await this.select('myTickets')
        } else if (this.lists.team.tickets.some((t) => ticketId(t) === id)) {
          this.setQuery('team', '')
          this.lists.team.selection = id
          this.syncDetails()
          await this.select('teamTickets')
        } else {
          this.setQuery('mine', ref)
          await this.select('myTickets')
        }
        return null
      }
      case 'openOrg':
        this.orgs.query = ''
        this.orgs.selectedId = intent.orgId
        await this.select('organizations')
        await this.loadOrgSelected()
        return null
      case 'openCI':
        await this.openCI({ key: intent.ci.key, className: intent.ci.className, name: intent.ci.name, orgName: '' })
        await this.select('configItems')
        return null
      case 'openOrgPeople':
        this.people.scope = { orgId: intent.orgId, orgName: intent.orgName }
        this.people.query = ''
        this.people.searched = null
        this.people.selectedId = null
        await this.select('people')
        await this.peopleSearch()
        return null
      case 'openOrgConfigItems':
        this.cmdb.scope = { orgId: intent.orgId, orgName: intent.orgName }
        this.cmdb.query = ''
        this.cmdb.searched = null
        await this.select('configItems')
        await this.cmdbSearch()
        return null
      case 'newTicket':
        await this.openNewTicket('sheet', { orgId: null, orgName: null, ciId: null, ciName: null })
        return null
      case 'newTicketForOrg':
        await this.openNewTicket('sheet', { orgId: intent.orgId, orgName: intent.orgName, ciId: null, ciName: null })
        return null
      case 'newTicketForCI':
        await this.openNewTicket('sheet', { orgId: null, orgName: intent.orgName, ciId: intent.key, ciName: intent.name })
        return null
      case 'setSidebarWidth':
        this.sidebarWidth = Math.min(260, Math.max(180, Math.round(intent.width)))
        this.changed()
        return null
      case 'setSidebarVisible':
        this.sidebarVisible = intent.visible
        this.changed()
        return null
    }
  }

  // ===========================================================================================
  // options
  // ===========================================================================================

  private summary(): string | null {
    const parts: string[] = []
    if (!this.isDefaultScope()) {
      const r = RANGE_TITLES[this.range].toLowerCase()
      parts.push(this.includeClosed ? (this.range === 0 ? 'All tickets, any time' : `All, ${r}`) : this.range === 0 ? 'Open tickets' : `Open, ${r}`)
    }
    if (this.sort !== 'priority') parts.push(SORT_TITLES[this.sort])
    if (this.pageSize !== 20) parts.push(this.pageSize === null ? 'All on one page' : `${this.pageSize} per page`)
    return parts.length === 0 ? null : parts.join(' · ')
  }

  private optionsSnapshot(): SnapshotOf<'options'> {
    const summary = this.summary()
    return {
      range: this.range, includeClosed: this.includeClosed, sort: this.sort, pageSize: this.pageSize,
      isDefault: summary === null, summary,
      help: summary ?? 'Open tickets, sorted by priority, 20 per page',
      accessibilityLabel: `View options: ${summary ?? 'default'}`,
      rangeChoices: ([0, 7, 30, 90] as RangeDays[]).map((value) => ({ value, title: RANGE_TITLES[value], selected: value === this.range })),
      sortChoices: (Object.keys(SORT_TITLES) as TicketSort[]).map((value) => ({ value, title: SORT_TITLES[value], selected: value === this.sort })),
      pageSizeChoices: PAGE_SIZES.map((value) => ({ value, title: value === null ? 'All' : `${value} per page`, selected: value === this.pageSize })),
    }
  }

  /** Only the shown view reacts, like the Swift views' onChange (see contract.md). */
  private async scopeChanged() {
    if (this.selection === 'myTickets' || this.selection === 'teamTickets') {
      const scope = this.selection === 'myTickets' ? 'mine' : 'team'
      this.lists[scope].page = 1
      await this.refresh(scope)
    } else if (this.selection === 'organizations') {
      await this.loadOrgSelected()
    } else if (this.selection === 'people') {
      await this.loadPersonSelected(true)
    }
  }

  private presentationChanged() {
    if (this.selection === 'myTickets') this.lists.mine.page = 1
    if (this.selection === 'teamTickets') this.lists.team.page = 1
    this.orgs.page = 1
    this.people.page = 1
  }

  private async optionsIntent(intent: IntentOf<'options'>): Promise<unknown> {
    switch (intent.type) {
      case 'setRange':
        this.range = intent.range
        await this.scopeChanged()
        return null
      case 'setIncludeClosed':
        this.includeClosed = intent.includeClosed
        await this.scopeChanged()
        return null
      case 'setSort':
        this.sort = intent.sort
        this.presentationChanged()
        return null
      case 'setPageSize':
        this.pageSize = intent.pageSize
        this.presentationChanged()
        return null
      case 'reset': {
        const scopeChanged = !this.isDefaultScope()
        this.range = 0
        this.includeClosed = false
        this.sort = 'priority'
        this.pageSize = 20
        this.presentationChanged()
        if (scopeChanged) await this.scopeChanged()
        return null
      }
    }
  }

  // ===========================================================================================
  // lists
  // ===========================================================================================

  private load(scope: ListScope): MockTicket[] {
    const now = this.now()
    const teamIds = ME.teams.map((t) => t.id)
    return [...this.db.values()]
      .filter((t) => this.includeClosed || !['resolved', 'closed'].includes(t.status))
      .filter((t) =>
        scope === 'mine'
          ? t.agentId === ME.contactId || (t.status === 'escalated_tto' && isUnassigned(t) && teamIds.includes(t.teamId))
          : teamIds.includes(t.teamId),
      )
      .filter((t) => this.range === 0 || (t.startDate ?? now).getTime() >= now.getTime() - this.range * 86_400_000)
      .map((t) => ({ ...t }))
      .sort((a, b) => Number(a.key) - Number(b.key))
  }

  private async refresh(scope: ListScope): Promise<void> {
    const list = this.lists[scope]
    if (list.refreshing) {
      list.refreshAgain = true
      return
    }
    if (scope === 'team' && ME.teams.length === 0) {
      list.phase = { type: 'loaded' }
      return
    }
    list.refreshing = true
    this.changed()
    if (this.options.listState === 'loading') return
    await this.wait()
    try {
      if (this.options.listState === 'error') throw new Error(NETWORK_ERROR)
      list.tickets = this.options.listState === 'empty' ? [] : this.load(scope)
      list.updatedAt = this.now()
      list.phase = { type: 'loaded' }
      list.refreshError = null
      this.refreshLookup(list)
      if (list.selection && !this.listTicket(list, list.selection)) list.selection = null
      if (scope === 'mine') await this.myTicketsChanged()
      this.syncDetails()
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (list.phase.type === 'loaded') list.refreshError = message
      else list.phase = { type: 'failed', message }
    } finally {
      list.refreshing = false
      this.changed()
      if (list.refreshAgain) {
        list.refreshAgain = false
        void this.refresh(scope)
      }
    }
  }

  private setLookup(list: ListState, lookup: Lookup) {
    list.lookup = lookup
    if (!(lookup.type === 'found' && ticketId(lookup.ticket) === list.justChanged)) list.justChanged = null
  }

  private refreshLookup(list: ListState) {
    if (list.lookup.type !== 'found') return
    const found = list.lookup.ticket
    const id = ticketId(found)
    if (list.justChanged === id && list.selection !== id) this.setLookup(list, { type: 'none' })
    else if (list.tickets.some((t) => ticketId(t) === id)) this.setLookup(list, { type: 'none' })
    else {
      const fresh = this.db.get(found.ref)
      if (fresh) this.setLookup(list, { type: 'found', ticket: { ...fresh } })
    }
  }

  private listTicket(list: ListState, id: string): MockTicket | null {
    if (list.lookup.type === 'found' && ticketId(list.lookup.ticket) === id) return list.lookup.ticket
    return list.tickets.find((t) => ticketId(t) === id) ?? null
  }

  private setQuery(scope: ListScope, query: string) {
    const list = this.lists[scope]
    list.query = query
    list.page = 1
    list.queryToken++
    void this.searchChanged(scope, list.queryToken)
  }

  private async searchChanged(scope: ListScope, token: number) {
    const list = this.lists[scope]
    const q = list.query.trim()
    const isRef = /^[RI]-\d{6}$/.test(q.toUpperCase())
    if (!isRef || list.tickets.some((t) => t.ref.toUpperCase() === q.toUpperCase())) {
      this.setLookup(list, { type: 'none' })
      return
    }
    const ref = q.toUpperCase()
    if (list.lookup.type === 'found' && list.lookup.ticket.ref === ref) return
    this.setLookup(list, { type: 'searching', ref })
    this.changed()
    await this.wait(250)
    if (list.queryToken !== token) return
    if (this.options.listState === 'error') {
      this.setLookup(list, { type: 'failed', message: NETWORK_ERROR })
    } else {
      const found = this.db.get(ref)
      if (list.query.trim().toUpperCase() !== ref) return
      this.setLookup(list, found ? { type: 'found', ticket: { ...found } } : { type: 'missing', ref })
    }
    this.syncDetails()
    this.changed()
  }

  private listSnapshot(scope: ListScope): SnapshotOf<'list:mine'> {
    const list = this.lists[scope]
    const searching = list.query.trim() !== ''
    const orgChoices = [...new Set(list.tickets.map((t) => t.orgName).filter(Boolean))].sort(natural)
    const visible = searchFilter(list.orgFilter ? list.tickets.filter((t) => t.orgName === list.orgFilter) : list.tickets, list.query)
    const all = groups(visible, scope, this.sort)
    const paged = paginate(all, this.pageSize, list.page)
    const noTeams = scope === 'team' && ME.teams.length === 0
    let content: ListContent
    switch (list.phase.type) {
      case 'loading':
        content = { type: 'loading' }
        break
      case 'failed':
        content = { type: 'failed', message: list.phase.message }
        break
      case 'loaded': {
        if (noTeams) {
          content = { type: 'message', text: "You're not in any iTop team, so there are no team tickets to show." }
        } else if (list.tickets.length === 0 && !searching) {
          content = {
            type: 'message',
            text: !this.isDefaultScope()
              ? 'No tickets in this range. Try a longer range in the View menu.'
              : scope === 'mine'
                ? 'Nothing waiting on you. New assignments show up here.'
                : 'Your teams have no open tickets.',
          }
        } else {
          const lookup = list.lookup
          const orgId = list.orgFilter ? list.tickets.find((t) => t.orgName === list.orgFilter && t.orgId !== '0')?.orgId : undefined
          content = {
            type: 'list',
            body: {
              orgNoMatch:
                list.orgFilter && all.length === 0 && !searching
                  ? {
                      text: `None of your tickets are for ${list.orgFilter}.`,
                      seeAll: orgId && scope === 'mine' ? { orgId, orgName: list.orgFilter } : null,
                    }
                  : null,
              refreshError: list.refreshError ? `Couldn't refresh: ${list.refreshError}` : null,
              lookupBar:
                lookup.type === 'searching'
                  ? { type: 'searching', text: `Looking up ${lookup.ref} in iTop…` }
                  : lookup.type === 'missing'
                    ? { type: 'missing', text: `${lookup.ref} doesn't exist, or your iTop account can't see it.` }
                    : lookup.type === 'failed'
                      ? { type: 'failed', text: `Couldn't look that up: ${lookup.message}` }
                      : null,
              showNotices: lookup.type === 'none' || lookup.type === 'found',
              notice: list.notice,
              extraSection:
                lookup.type === 'found'
                  ? {
                      title:
                        list.justChanged === ticketId(lookup.ticket)
                          ? `${statusLabel(lookup.ticket.status)} just now`
                          : 'Search result: not in your queue',
                      row: this.row(lookup.ticket, scope, true),
                    }
                  : null,
              noMatchText:
                searching && paged.sections.length === 0 && lookup.type === 'none'
                  ? `No tickets in your list match “${list.query.trim()}”.`
                  : null,
              sections: paged.sections.map(
                (s): TicketSection => ({
                  group: s.group, title: GROUP_TITLES[s.group], total: paged.totals.get(s.group) ?? 0,
                  rows: s.tickets.map((t) => this.row(t, scope, false)),
                }),
              ),
            },
          }
        }
      }
    }
    const minutes = list.updatedAt ? Math.floor((this.now().getTime() - list.updatedAt.getTime()) / 60000) : 0
    return {
      scope,
      query: list.query,
      selection: list.selection,
      content,
      showControls: list.phase.type === 'loaded' && !noTeams,
      orgFilter: {
        visible: scope === 'mine' && (orgChoices.length > 1 || list.orgFilter !== null),
        selected: list.orgFilter,
        label: list.orgFilter ?? 'All organizations',
        help: list.orgFilter ? `Showing tickets for ${list.orgFilter}` : 'Filter by organization',
        accessibilityLabel: `Filter by organization: ${list.orgFilter ?? 'all'}`,
        choices: orgChoices.map((name) => ({ name, selected: name === list.orgFilter })),
      },
      pager: list.phase.type === 'loaded' ? paged.pager : { type: 'none' },
      refreshing: list.refreshing,
      footerSpinner: list.refreshing && list.phase.type === 'loaded',
      updatedText: list.updatedAt
        ? minutes < 1
          ? 'Updated just now'
          : `Updated ${minutes} min ago`
        : list.phase.type === 'loading'
          ? 'Loading'
          : 'Not updated yet',
      needCount: list.tickets.filter((t) => this.needsYou(t)).length,
      working: list.working,
    }
  }

  private async listIntent(scope: ListScope, intent: IntentOf<'list:mine'>): Promise<unknown> {
    const list = this.lists[scope]
    switch (intent.type) {
      case 'setQuery':
        this.setQuery(scope, intent.query)
        return null
      case 'select':
        list.selection = intent.ticketId
        this.syncDetails()
        return null
      case 'setPage':
        list.page = intent.page
        return null
      case 'toggleOrg':
        list.orgFilter = list.orgFilter === intent.name ? null : intent.name
        list.page = 1
        return null
      case 'clearOrgFilter':
        list.orgFilter = null
        list.page = 1
        return null
      case 'refresh':
        await this.refresh(scope)
        return null
      case 'clearNotice':
        list.notice = null
        return null
      case 'rowAssignToMe': {
        const t = this.listTicket(list, intent.ticketId)
        if (!t) return null
        list.selection = intent.ticketId
        this.syncDetails()
        await this.assignToMe(scope, t)
        return null
      }
    }
  }

  // --- writes ---------------------------------------------------------------------------------

  private actionError(message: string, ref: string, verb: string): string {
    return `iTop didn't ${verb.replace('<ref>', ref)}. ${withPeriod(message)}`
  }

  private async assignToMe(scope: ListScope, t: MockTicket): Promise<void> {
    const list = this.lists[scope]
    if (list.working) return
    list.working = ticketId(t)
    list.notice = null
    this.changed()
    try {
      await this.wait()
      if (!this.canWrite) throw new Error('writesOff')
      if (t.ref === RACE_REF) {
        list.notice = { text: `Agent 3 took ${t.ref} a moment ago, so it stays with them.`, isError: true }
        await this.refresh(scope)
        return
      }
      if (!this.canAssign(t)) {
        list.notice = { text: `${statusLabel(t.status)} tickets can't be assigned.`, isError: true }
        return
      }
      const row = this.db.get(t.ref)!
      const team = ME.teams.find((x) => x.id === row.teamId) ?? ME.teams[0]!
      Object.assign(row, { agentId: ME.contactId, agentName: ME.displayName, teamId: team.id, teamName: team.name, status: 'assigned', lastUpdate: this.now() })
      list.notice = { text: `${t.ref} is assigned to you.`, isError: false }
      await this.refresh(scope)
    } catch {
      list.notice = { text: 'Changes to iTop are switched off in this build, so nothing was sent.', isError: true }
    } finally {
      list.working = null
    }
  }

  private async assignTo(scope: ListScope, t: MockTicket, team: Ref, agent: Ref): Promise<string | null> {
    const list = this.lists[scope]
    if (list.working) return 'Another assignment is still running.'
    list.working = ticketId(t)
    list.notice = null
    this.changed()
    try {
      await this.wait()
      if (!this.canWrite) return 'Changes to iTop are switched off in this build, so nothing was sent.'
      if (!this.canAssign(t)) return `${statusLabel(t.status)} tickets can't be assigned.`
      const row = this.db.get(t.ref)!
      const next = ['new', 'escalated_tto', 'pending'].includes(row.status) ? 'assigned' : row.status
      Object.assign(row, { teamId: team.id, teamName: team.name, agentId: agent.id, agentName: agent.name, status: next, lastUpdate: this.now() })
      const who = agent.id === ME.contactId ? 'you' : agent.name
      list.notice = { text: `${t.ref} is assigned to ${who} in ${team.name}.`, isError: false }
      await this.refresh(scope)
      return null
    } finally {
      list.working = null
    }
  }

  private async apply(
    scope: ListScope,
    t: MockTicket,
    change: { action: 'pending'; reason: string } | { action: 'resolve'; solution: string } | { action: 'reopen' } | { action: 'close' },
  ): Promise<string | null> {
    const list = this.lists[scope]
    if (list.working) return 'Another change is still running.'
    list.working = ticketId(t)
    list.notice = null
    this.changed()
    const verbs = { pending: 'set <ref> to pending', resolve: 'resolve <ref>', reopen: 'reopen <ref>', close: 'close <ref>' }
    try {
      if (change.action === 'pending' && this.timer.running?.ticketId === ticketId(t)) await this.stopTimer(`Setting ${t.ref} to pending stopped its timer.`)
      if (change.action === 'resolve' && this.timer.running?.ticketId === ticketId(t)) await this.stopTimer(`Resolving ${t.ref} stopped its timer.`)
      await this.wait()
      if (!this.canWrite) return 'Changes to iTop are switched off in this build, so nothing was sent.'
      const row = this.db.get(t.ref)!
      if (!actionsFor(row.status).includes(change.action)) {
        return `${statusLabel(row.status)} tickets can't do that. Refresh to see the ticket as it is now.`
      }
      if (change.action === 'pending' && /fail/i.test(change.reason)) return this.actionError(SERVER_ERROR('Error: demo failure on pending'), t.ref, verbs.pending)
      if (change.action === 'resolve' && /fail/i.test(change.solution)) {
        return this.actionError(SERVER_ERROR("Error: servicesubcategory_id: the subcategory doesn't belong to this service"), t.ref, verbs.resolve)
      }
      const next = { pending: 'pending', resolve: 'resolved', reopen: 'assigned', close: 'closed' }[change.action]
      Object.assign(row, { status: next, lastUpdate: this.now() })
      const done = { pending: 'set to pending', resolve: 'resolved', reopen: 'reopened', close: 'closed' }[change.action]
      list.notice = { text: `${t.ref} is ${done}.`, isError: false }
      if (list.selection === ticketId(t) && (change.action === 'resolve' || change.action === 'close')) {
        this.setLookup(list, { type: 'found', ticket: { ...row } })
        list.justChanged = ticketId(t)
      }
      await this.refresh(scope)
      return null
    } finally {
      list.working = null
    }
  }

  // ===========================================================================================
  // details
  // ===========================================================================================

  private signature(t: MockTicket) {
    return `${t.status}|${t.agentId}|${t.teamId}|${t.lastUpdate?.getTime() ?? 0}`
  }

  /** Creates, drops or reloads the detail models to follow each list's selection. */
  private syncDetails() {
    for (const scope of ['mine', 'team'] as ListScope[]) {
      const list = this.lists[scope]
      const t = list.selection ? this.listTicket(list, list.selection) : null
      const current = this.detail[scope]
      if (!t) {
        this.detail[scope] = null
        continue
      }
      const id = ticketId(t)
      if (!current || current.ticketId !== id) {
        this.detail[scope] = {
          ticketId: id, fallback: { ...t }, signature: this.signature(t), phase: { type: 'loading' }, tab: 'public', copied: false,
          sending: false, sendError: null, attachments: { type: 'loading' }, loadToken: 0,
        }
        void this.loadDetail(scope)
        void this.loadAttachments(scope, t)
        this.checkWebStopwatch(t)
      } else if (current.signature !== this.signature(t)) {
        current.signature = this.signature(t)
        void this.loadDetail(scope)
      }
    }
  }

  private detailTicket(scope: ListScope): MockTicket | null {
    const d = this.detail[scope]
    if (!d) return null
    return this.listTicket(this.lists[scope], d.ticketId) ?? d.fallback
  }

  private async loadDetail(scope: ListScope) {
    const d = this.detail[scope]
    const t = this.detailTicket(scope)
    if (!d || !t) return
    const token = ++d.loadToken
    if (d.phase.type !== 'loaded') d.phase = { type: 'loading' }
    this.changed()
    await this.wait()
    if (this.detail[scope] !== d || d.loadToken !== token) return
    if (this.options.listState === 'error') {
      if (d.phase.type !== 'loaded') d.phase = { type: 'failed', message: NETWORK_ERROR }
    } else {
      const base = this.details[t.ref] ?? emptyDetail()
      const extra = this.added[t.ref] ?? []
      const entries = (priv: boolean, from: number) =>
        extra.filter((e) => e.isPrivate === priv).map((e, i) => ({ id: from + i, date: e.date, author: ME.displayName, text: e.text, images: e.images }))
      d.phase = {
        type: 'loaded',
        detail: { ...base, publicLog: [...base.publicLog, ...entries(false, 1000)], privateLog: [...base.privateLog, ...entries(true, 2000)] },
      }
    }
    this.changed()
  }

  private async loadAttachments(scope: ListScope, t: MockTicket) {
    await this.wait()
    const d = this.detail[scope]
    if (!d || d.ticketId !== ticketId(t)) return
    d.attachments = { type: 'loaded', items: ATTACHMENTS[t.ref] ?? [], downloadingId: null, saved: null, error: null }
    this.changed()
  }

  private richOf(text: string, images: string[], ticketUrl: string): RichContent {
    return {
      blocks: [
        ...(text ? [{ type: 'text' as const, runs: [{ type: 'text' as const, text, link: null }] }] : []),
        ...images.map((src) => ({ type: 'image' as const, src, ticketWebUrl: ticketUrl })),
      ],
    }
  }

  private timerButton(t: MockTicket): TimerButton {
    const id = ticketId(t)
    if (this.timer.running?.ticketId === id) return { type: 'running' }
    const web = this.timer.web[id]
    if (web) return { type: 'webStopwatch', label: `Web stopwatch since ${sinceText(web, this.now())}` }
    if (this.canStart(t)) {
      const running = this.timer.running
      return {
        type: 'start',
        enabled: this.canWrite && !this.timer.saving,
        help: this.canWrite ? (running ? `Start timer: stops the one on ${running.ref} and saves its time (⌘T)` : 'Start timer (⌘T)') : WRITES_OFF,
      }
    }
    return { type: 'none' }
  }

  private detailSnapshot(scope: ListScope): DetailSnapshot {
    const d = this.detail[scope]
    const t = this.detailTicket(scope)
    if (!d || !t) return { type: 'none', placeholder: 'Select a ticket to see its details.' }
    const list = this.lists[scope]
    const id = ticketId(t)
    const url = this.webUrl(t.ticketClass, t.key)
    const needs = this.needsYou(t)
    const primaryKind = primaryFor(t.status)
    const disabled = list.working !== null || !this.canWrite
    const help = (text: string) => (this.canWrite ? text : WRITES_OFF)
    let primary: PrimaryAction = { type: 'none' }
    const action = (kind: 'assignToMe' | 'openAssign' | 'openResolve' | 'confirmClose', label: string, text: string, teal: boolean): PrimaryAction => ({
      type: 'action', kind, label, help: help(text), teal, enabled: !disabled, busy: list.working === id,
    })
    if (primaryKind === 'assign') {
      primary = isUnassigned(t) ? action('assignToMe', 'Assign to me', `Assign ${t.ref} to you`, needs) : action('openAssign', 'Assign ticket', 'Choose a team and agent', needs)
    } else if (primaryKind === 'resolve') primary = action('openResolve', 'Resolve ticket', `Resolve ${t.ref}`, needs)
    else if (primaryKind === 'close') primary = action('confirmClose', 'Close ticket', `Close ${t.ref}`, false)
    const actions = actionsFor(t.status)
    const spent = this.timer.totals[id] ?? 0
    const loaded = d.phase.type === 'loaded' ? d.phase.detail : null
    const key = `${id}|${d.tab}`
    const draft = list.drafts[key] ?? ''
    const images = list.draftImages[key] ?? []
    const isPrivate = d.tab === 'private'
    let status: Notice | null = null
    if (d.sendError) status = { text: d.sendError, isError: true }
    else if (d.sending && images.length > 0) {
      status = { text: images.length === 1 ? 'Uploading the image to iTop' : `Uploading ${images.length} images to iTop`, isError: false }
    } else if (!this.canWrite) status = { text: 'Adding entries is switched off in this build, so nothing is sent to iTop.', isError: false }
    const canAddImages = this.canWrite && !d.sending && images.length < MAX_IMAGES
    const composer: Composer = {
      isPrivate, draft,
      images: images.map((im) => ({ id: im.id, filename: im.filename, sizeText: byteCount(im.size), thumbnailSrc: im.dataUrl })),
      imagesAccessibilityLabel: images.length === 1 ? '1 image to add' : `${images.length} images to add`,
      placeholder: isPrivate ? 'Write a private entry, only agents see it' : 'Write a public entry, the caller sees it',
      accessibilityLabel: isPrivate ? 'Private entry' : 'Public entry',
      sendLabel: isPrivate ? 'Add to private log' : 'Add to public log',
      sendHelp: this.canWrite ? 'Add the entry (⌘↩)' : WRITES_OFF,
      editable: this.canWrite,
      canSend: this.canWrite && !d.sending && (draft.trim() !== '' || images.length > 0),
      sending: d.sending,
      canAddImages,
      paperclipHelp: canAddImages
        ? 'Add images. You can also paste or drop them in the entry. PNG, JPEG or GIF, up to 4 MB each.'
        : images.length >= MAX_IMAGES
          ? 'An entry can have up to 5 images'
          : 'Adding images is switched off',
      status,
      maxIntakeFileBytes: MAX_INTAKE_BYTES,
    }
    const entries = loaded ? (isPrivate ? loaded.privateLog : loaded.publicLog) : []
    return {
      type: 'ticket',
      ticket: this.ticketView(t),
      classLabel: t.ticketClass === 'UserRequest' ? 'User request' : 'Incident',
      copied: d.copied,
      timerButton: this.timerButton(t),
      primary,
      more: {
        assign:
          this.canAssign(t) && !(primaryKind === 'assign' && !isUnassigned(t))
            ? { label: isUnassigned(t) ? 'Assign to someone else…' : 'Reassign…' }
            : null,
        pending: actions.includes('pending'),
        reopen: actions.includes('reopen'),
        edit: t.status !== 'closed',
        enabled: !disabled,
        divider: actions.length > 0 || t.status !== 'closed',
        webUrl: url,
      },
      closeConfirm: { title: `Close ${t.ref}?`, message: "A closed ticket can't be reopened from the app." },
      strip: {
        caller: t.callerName || 'None',
        organization: t.orgName || 'None',
        team: t.teamName || 'No team',
        agent: this.agentChip(t),
        spent: spent > 0 ? `${total(spent)} spent` : null,
        linkedCIs: loaded ? loaded.linkedCIs : [],
      },
      body:
        d.phase.type === 'loading'
          ? { type: 'loading' }
          : d.phase.type === 'failed'
            ? { type: 'failed', text: `Couldn't load this ticket. ${d.phase.message}` }
            : {
                type: 'loaded',
                serviceLine: loaded!.serviceName ? [loaded!.serviceName, loaded!.subcategoryName].filter(Boolean).join(' › ') : null,
                description: loaded!.descriptionBlocks
                  ? { blocks: loaded!.descriptionBlocks }
                  : loaded!.descriptionText
                    ? this.richOf(loaded!.descriptionText, [], url)
                    : null,
                tab: d.tab,
                entries: entries.map((e) => ({
                  id: e.id,
                  header: [e.author, e.date ? logDate(e.date, this.now()) : ''].filter(Boolean).join(' · '),
                  isPrivate,
                  content: this.richOf(e.text, e.images, url),
                })),
                emptyLogText:
                  entries.length > 0 ? null : isPrivate ? 'No private entries yet. Only agents can see the private log.' : 'No public entries yet.',
              },
      attachments: d.attachments,
      composer: loaded ? composer : null,
    }
  }

  private intake(d: DetailState, items: ImageIntakeItem[], existing: number): MockImage[] {
    const accepted: MockImage[] = []
    const problems: string[] = []
    for (const item of items) {
      const name = item.filename || 'Pasted image.png'
      if (item.dataBase64 === null) {
        problems.push(`${name} is larger than 4 MB.`)
        continue
      }
      const bytes = base64Bytes(item.dataBase64)
      const mime = sniffImage(bytes)
      if (mime && bytes.length <= MAX_IMAGE_BYTES) {
        accepted.push({ id: crypto.randomUUID(), filename: name, size: bytes.length, dataUrl: `data:${mime};base64,${item.dataBase64}` })
      } else if (mime) {
        problems.push(`${name} is larger than 4 MB.`)
      } else {
        problems.push(`${item.filename || 'That file'} isn't an image iTop Wrap can add. Use PNG, JPEG or GIF.`)
      }
    }
    const room = Math.max(0, MAX_IMAGES - existing)
    if (accepted.length > room) {
      const left = accepted.length - room
      problems.push(`An entry can have up to 5 images, so ${left} ${left === 1 ? "wasn't" : "weren't"} added.`)
      accepted.length = room
    }
    d.sendError = problems.length === 0 ? null : problems.join(' ')
    return accepted
  }

  private async send(scope: ListScope): Promise<boolean> {
    const d = this.detail[scope]
    const t = this.detailTicket(scope)
    if (!d || !t || d.sending) return false
    const list = this.lists[scope]
    const key = `${d.ticketId}|${d.tab}`
    const text = list.drafts[key] ?? ''
    const images = list.draftImages[key] ?? []
    const isPrivate = d.tab === 'private'
    d.sending = true
    d.sendError = null
    this.changed()
    try {
      await this.wait(400 + 900 * images.length)
      if (text.trim() === '' && images.length === 0) {
        d.sendError = 'Write something or add an image first.'
        return false
      }
      if (images.length > MAX_IMAGES) {
        d.sendError = 'An entry can have up to 5 images. Remove some, then add the entry. Your draft is kept.'
        return false
      }
      if (!this.canWrite) {
        d.sendError = 'Changes to iTop are switched off in this build, so nothing was sent. Your draft is kept.'
        return false
      }
      if (/fail/i.test(text)) {
        d.sendError = `iTop didn't save the entry. ${withPeriod(SERVER_ERROR('Error: demo failure on request'))} Your draft is kept.`
        return false
      }
      if (images.length > 0 && /refuse/i.test(text)) {
        d.sendError = `iTop didn't save the entry. ${withPeriod(SERVER_ERROR('Error: contents: demo refusal, the image was not stored'))} Your draft is kept.`
        return false
      }
      ;(this.added[t.ref] ??= []).push({ text: text.trim(), images: images.map((i) => i.dataUrl), isPrivate, date: this.now() })
      delete list.drafts[key]
      delete list.draftImages[key]
      d.sending = false
      await this.loadDetail(scope)
      return true
    } finally {
      d.sending = false
    }
  }

  private async detailIntent(scope: ListScope, intent: IntentOf<'detail:mine'>): Promise<unknown> {
    const d = this.detail[scope]
    const t = this.detailTicket(scope)
    if (!d || !t) throw new BackendError({ type: 'invalidIntent', message: `detail:${scope} has no ticket` })
    const list = this.lists[scope]
    const key = `${d.ticketId}|${d.tab}`
    switch (intent.type) {
      case 'retry':
        await this.loadDetail(scope)
        return null
      case 'setTab':
        d.tab = intent.tab
        return null
      case 'copyRef':
        try {
          await navigator.clipboard?.writeText(t.ref)
        } catch {
          // The mock has no clipboard in tests or without permission.
        }
        d.copied = true
        setTimeout(() => {
          d.copied = false
          this.changed()
        }, 2000)
        return null
      case 'assignToMe':
        await this.assignToMe(scope, t)
        return null
      case 'edit':
        if (t.status !== 'closed') await this.openEdit(t)
        return null
      case 'reopen':
      case 'close': {
        const message = await this.apply(scope, t, { action: intent.type })
        if (message) list.notice = { text: message, isError: true }
        return null
      }
      case 'toggleTimer': {
        const button = this.timerButton(t)
        if (button.type === 'running') await this.stopTimer(null)
        else if (button.type === 'start' && button.enabled) await this.startTimer(t)
        return null
      }
      case 'setDraft':
        list.drafts[key] = intent.text
        return null
      case 'addImages': {
        const existing = list.draftImages[key] ?? []
        const accepted = this.intake(d, intent.items, existing.length)
        if (accepted.length > 0) list.draftImages[key] = [...existing, ...accepted]
        return null
      }
      // A browser has no NSPasteboard: the UI takes images from the paste event itself.
      case 'pasteFromClipboard':
        return 'text'
      case 'removeImage': {
        const rest = (list.draftImages[key] ?? []).filter((i) => i.id !== intent.imageId)
        if (rest.length === 0) delete list.draftImages[key]
        else list.draftImages[key] = rest
        d.sendError = null
        return null
      }
      case 'send':
        return this.send(scope)
      case 'downloadAttachment': {
        const a = d.attachments
        if (a.type !== 'loaded') return null
        const item = a.items.find((i) => i.id === intent.attachmentId)
        if (!item) return null
        d.attachments = { ...a, downloadingId: item.id, saved: null, error: null }
        this.changed()
        await this.wait()
        const body = `Demo attachment ${item.name}\n`
        d.attachments = { ...a, downloadingId: null, saved: `Saved ${item.name} (${byteCount(body.length)}).`, error: null }
        return null
      }
    }
  }

  // ===========================================================================================
  // sheets
  // ===========================================================================================

  private sheetTicket(scope: ListScope, id: string): MockTicket {
    const t = this.listTicket(this.lists[scope], id) ?? this.findTicket(id)
    if (!t) throw new BackendError({ type: 'invalidIntent', message: `No ticket ${id}` })
    return t
  }

  private timerLine(t: MockTicket): string | null {
    const r = this.timer.running
    if (!r || r.ticketId !== ticketId(t)) return null
    return `Your timer stops and saves ${clock((this.now().getTime() - r.start.getTime()) / 1000)} to this ticket.`
  }

  private assignSnapshot(scope: ListScope): SnapshotOf<'assign:mine'> {
    const s = this.assign[scope]
    if (!s) return { type: 'closed' }
    const t = this.sheetTicket(scope, s.ticketId)
    const busy = this.lists[scope].working !== null
    const teams = valueOf(s.teams)
    const agents = valueOf(s.agents)
    return {
      type: 'open',
      ticketId: s.ticketId,
      title: isUnassigned(t) ? `Assign ${t.ref}` : `Reassign ${t.ref}`,
      subtitle: t.title,
      assignToMe: { enabled: !busy && t.agentId !== ME.contactId, help: t.agentId === ME.contactId ? "It's already yours" : `Assign ${t.ref} to you` },
      team: { items: pick(teams ?? []), selectedId: s.teamId, placeholder: placeholderOf(s.teams, 'Loading teams…', 'No teams found'), enabled: teams !== null, search: null },
      agent: {
        items: s.teamId === null ? [] : (agents ?? []).map((a) => ({ id: a.id, label: a.id === ME.contactId ? `${a.name} (you)` : a.name })),
        selectedId: s.agentId,
        placeholder: s.teamId === null ? 'Choose a team first' : placeholderOf(s.agents, 'Loading agents…', 'Nobody in this team'),
        enabled: s.teamId !== null && agents !== null,
        search: null,
      },
      confirmLabel: isUnassigned(t) ? 'Assign ticket' : 'Reassign ticket',
      confirmEnabled: !busy && s.teamId !== null && s.agentId !== null,
      busy,
      error: s.error,
    }
  }

  private async loadAgents(s: AssignState, teamId: string) {
    s.agents = { type: 'loading' }
    this.changed()
    await this.wait(200)
    if (s.teamId !== teamId) return
    s.agents = { type: 'loaded', value: AGENTS[teamId] ?? [] }
    this.changed()
  }

  private async assignIntent(scope: ListScope, intent: IntentOf<'assign:mine'>): Promise<unknown> {
    if (intent.type === 'open') {
      const t = this.sheetTicket(scope, intent.ticketId)
      const s: AssignState = { ticketId: intent.ticketId, teams: { type: 'loading' }, agents: { type: 'idle' }, teamId: null, agentId: null, error: null }
      this.assign[scope] = s
      this.changed()
      await this.wait(200)
      s.teams = { type: 'loaded', value: TEAMS }
      const own = TEAMS.find((x) => x.id === t.teamId)
      if (own) {
        s.teamId = own.id
        await this.loadAgents(s, own.id)
      }
      return null
    }
    const s = this.assign[scope]
    if (!s) return intent.type === 'assignToMe' || intent.type === 'confirm' ? 'The sheet is closed.' : null
    const t = this.sheetTicket(scope, s.ticketId)
    switch (intent.type) {
      case 'close':
        this.assign[scope] = null
        return null
      case 'setTeam':
        s.teamId = intent.teamId
        s.agentId = null
        if (intent.teamId) await this.loadAgents(s, intent.teamId)
        return null
      case 'setAgent':
        s.agentId = intent.agentId
        return null
      case 'assignToMe': {
        await this.assignToMe(scope, t)
        const notice = this.lists[scope].notice
        if (notice && !notice.isError) {
          this.assign[scope] = null
          return null
        }
        s.error = notice?.text ?? null
        return s.error
      }
      case 'confirm': {
        const team = TEAMS.find((x) => x.id === s.teamId)
        const agent = (AGENTS[s.teamId ?? ''] ?? []).find((a) => a.id === s.agentId)
        if (!team || !agent) return null
        const message = await this.assignTo(scope, t, team, agent)
        if (message) s.error = message
        else this.assign[scope] = null
        return message
      }
    }
  }

  private pendingSnapshot(scope: ListScope): SnapshotOf<'pending:mine'> {
    const s = this.pending[scope]
    if (!s) return { type: 'closed' }
    const t = this.sheetTicket(scope, s.ticketId)
    const list = this.lists[scope]
    const empty = s.reason.trim() === ''
    return {
      type: 'open', ticketId: s.ticketId, title: `Set ${t.ref} to pending`, subtitle: t.title, reason: s.reason,
      reasonRequired: { missing: s.tried && empty }, reasonError: s.tried && empty ? 'Enter why the ticket is waiting.' : null,
      timerLine: this.timerLine(t), error: s.error, busy: list.working === s.ticketId, confirmEnabled: list.working === null,
    }
  }

  private async pendingIntent(scope: ListScope, intent: IntentOf<'pending:mine'>): Promise<unknown> {
    if (intent.type === 'open') {
      this.sheetTicket(scope, intent.ticketId)
      this.pending[scope] = { ticketId: intent.ticketId, reason: '', tried: false, error: null }
      return null
    }
    const s = this.pending[scope]
    if (!s) return intent.type === 'confirm' ? 'The sheet is closed.' : null
    switch (intent.type) {
      case 'close':
        this.pending[scope] = null
        return null
      case 'setReason':
        s.reason = intent.reason
        return null
      case 'confirm': {
        s.tried = true
        if (s.reason.trim() === '') return null
        const message = await this.apply(scope, this.sheetTicket(scope, s.ticketId), { action: 'pending', reason: s.reason.trim() })
        if (message) s.error = message
        else this.pending[scope] = null
        return message
      }
    }
  }

  private resolveSnapshot(slot: ResolveSlot): SnapshotOf<'resolve:mine'> {
    const scope: ListScope = slot === 'window' ? 'mine' : slot
    const s = this.resolve[slot]
    if (!s) return { type: 'closed' }
    const t = this.sheetTicket(scope, s.ticketId)
    const list = this.lists[scope]
    const services = valueOf(s.services)
    const subs = valueOf(s.subcategories)
    const solutionEmpty = s.solution.trim() === ''
    const plain = (items: PickItem[], selectedId: string | null, placeholder: string, enabled: boolean): Dropdown => ({
      items, selectedId, placeholder, enabled, search: null,
    })
    return {
      type: 'open', ticketId: s.ticketId, title: `Resolve ${t.ref}`, subtitle: t.title,
      service: plain(pick(services ?? []), s.serviceId, placeholderOf(s.services, 'Loading services…', 'No services found'), services !== null),
      serviceRequired: { missing: s.tried && s.serviceId === null },
      subcategory: plain(
        s.serviceId === null ? [] : pick(subs ?? []),
        s.subcategoryId,
        s.serviceId === null ? 'Choose a service first' : placeholderOf(s.subcategories, 'Loading subcategories…', 'This service has no subcategories'),
        s.serviceId !== null && subs !== null,
      ),
      subcategoryRequired: { missing: s.tried && s.subcategoryId === null },
      code: plain(RESOLUTION_CODES.map((c) => ({ id: c, label: c.slice(0, 1).toUpperCase() + c.slice(1) })), s.codeId, 'Choose…', true),
      solution: s.solution,
      solutionRequired: { missing: s.tried && solutionEmpty },
      solutionError: s.tried && solutionEmpty ? 'Describe what fixed it. The caller sees this.' : null,
      timerLine: this.timerLine(t), error: s.error, busy: list.working === s.ticketId, confirmEnabled: list.working === null,
    }
  }

  private async loadSubcategories(s: ResolveState, serviceId: string, keep: Ref | null) {
    s.subcategories = { type: 'loading' }
    this.changed()
    await this.wait(200)
    if (s.serviceId !== serviceId) return
    const found = [...(SUBCATEGORIES[serviceId] ?? [])]
    if (keep && !found.some((x) => x.id === keep.id)) found.unshift(keep)
    s.subcategories = { type: 'loaded', value: found }
    this.changed()
  }

  private async resolveIntent(slot: ResolveSlot, intent: IntentOf<'resolve:mine'>): Promise<unknown> {
    const scope: ListScope = slot === 'window' ? 'mine' : slot
    if (intent.type === 'open') {
      const t = this.sheetTicket(scope, intent.ticketId)
      const s: ResolveState = {
        ticketId: intent.ticketId, services: { type: 'loading' }, subcategories: { type: 'idle' }, serviceId: null, subcategoryId: null,
        codeId: RESOLUTION_CODES[0]!, solution: '', tried: false, error: null,
      }
      this.resolve[slot] = s
      this.changed()
      await this.wait(200)
      const all = [...SERVICES]
      const d = this.details[t.ref]
      if (d && isID(d.serviceId)) {
        if (!all.some((x) => x.id === d.serviceId)) all.unshift({ id: d.serviceId, name: d.serviceName || `Service ${d.serviceId}` })
        const keep = isID(d.subcategoryId) ? { id: d.subcategoryId, name: d.subcategoryName || `Subcategory ${d.subcategoryId}` } : null
        s.services = { type: 'loaded', value: all }
        s.serviceId = d.serviceId
        s.subcategoryId = keep?.id ?? null
        await this.loadSubcategories(s, d.serviceId, keep)
      } else {
        s.services = { type: 'loaded', value: all }
      }
      return null
    }
    const s = this.resolve[slot]
    if (!s) return intent.type === 'confirm' || intent.type === 'confirmAndClose' ? 'The sheet is closed.' : null
    switch (intent.type) {
      case 'close':
        this.resolve[slot] = null
        return null
      case 'setService':
        s.serviceId = intent.serviceId
        s.subcategoryId = null
        s.subcategories = { type: 'idle' }
        if (intent.serviceId) await this.loadSubcategories(s, intent.serviceId, null)
        return null
      case 'setSubcategory':
        s.subcategoryId = intent.subcategoryId
        return null
      case 'setCode':
        s.codeId = intent.codeId
        return null
      case 'setSolution':
        s.solution = intent.solution
        return null
      case 'confirm':
      case 'confirmAndClose': {
        s.tried = true
        if (!s.serviceId || !s.subcategoryId || !s.codeId || s.solution.trim() === '') return null
        const t = this.sheetTicket(scope, s.ticketId)
        const message = await this.apply(scope, t, { action: 'resolve', solution: s.solution })
        if (message) s.error = message
        else this.resolve[slot] = null
        if (!message && intent.type === 'confirmAndClose') {
          // Resolved; a refused Close leaves it resolved and goes to the list notice.
          const closeMessage = await this.apply(scope, t, { action: 'close' })
          if (closeMessage) this.lists[scope].notice = { text: closeMessage, isError: true }
        }
        return message
      }
    }
  }

  // ===========================================================================================
  // new ticket
  // ===========================================================================================

  private async openNewTicket(which: 'sheet' | 'window', start: NewTicketState['start']) {
    const plain = start.orgId === null && start.ciId === null
    const draft = plain && this.newTicketDraft ? { ...this.newTicketDraft } : blankDraft()
    const s: NewTicketState = {
      ...draft, start, orgs: { type: 'loading' }, callers: { type: 'idle' }, services: { type: 'idle' }, subcategories: { type: 'idle' },
      tried: false, creating: false, error: null, orgId: null, callerId: null, serviceId: null, subcategoryId: null,
    }
    this.newTicket[which] = s
    this.changed()
    await this.wait(200)
    if (this.newTicket[which] !== s) return
    s.orgs = { type: 'loaded', value: NEW_TICKET_ORGS }
    const wanted = start.orgId ?? draft.orgId
    const org = NEW_TICKET_ORGS.find((o) => o.id === wanted) ?? NEW_TICKET_ORGS.find((o) => o.name === start.orgName)
    if (!org) return
    s.orgId = org.id
    await this.loadForOrg(s, org.id)
    s.callerId = (valueOf(s.callers) ?? []).find((c) => c.id === draft.callerId)?.id ?? null
    const service = (valueOf(s.services) ?? []).find((x) => x.id === draft.serviceId)
    if (!service) return
    s.serviceId = service.id
    await this.loadNewSubcategories(s, service.id)
    s.subcategoryId = (valueOf(s.subcategories) ?? []).find((x) => x.id === draft.subcategoryId)?.id ?? null
  }

  private async loadForOrg(s: NewTicketState, orgId: string) {
    s.callers = { type: 'loading' }
    s.services = { type: 'loading' }
    this.changed()
    await this.wait(200)
    if (s.orgId !== orgId) return
    s.callers = { type: 'loaded', value: CALLERS[orgId] ?? [] }
    s.services = { type: 'loaded', value: SERVICES }
    this.changed()
  }

  private async loadNewSubcategories(s: NewTicketState, serviceId: string) {
    s.subcategories = { type: 'loading' }
    this.changed()
    await this.wait(200)
    if (s.serviceId !== serviceId) return
    s.subcategories = { type: 'loaded', value: SUBCATEGORIES[serviceId] ?? [] }
    this.changed()
  }

  private firstMissing(s: NewTicketState): string | null {
    if (!s.orgId) return 'organization'
    if (!s.callerId) return 'caller'
    if (!s.serviceId) return 'service'
    if (!s.subcategoryId) return 'subcategory'
    if (s.title.trim() === '') return 'title'
    if (s.description.trim() === '') return 'description'
    return null
  }

  /** Edit ticket on a mock ticket: prefilled; R-000412's description has an image, so it's locked. */
  private async openEdit(t: MockTicket) {
    const org = NEW_TICKET_ORGS.find((o) => o.name === t.orgName) ?? NEW_TICKET_ORGS[0]!
    const locked = t.ref === 'R-000412'
    const original: NewTicketDraft = {
      ticketClass: t.ticketClass, orgId: org.id, callerId: (CALLERS[org.id] ?? [])[0]?.id ?? null,
      serviceId: SERVICES[0]?.id ?? null, subcategoryId: null, impact: '2', urgency: '3', title: t.title,
      description: locked ? '' : 'The caller reports the problem started this morning.',
    }
    original.subcategoryId = (SUBCATEGORIES[original.serviceId ?? ''] ?? [])[0]?.id ?? null
    const s: NewTicketState = {
      ...original, start: { orgId: null, orgName: null, ciId: null, ciName: null }, orgs: { type: 'loading' }, callers: { type: 'idle' },
      services: { type: 'idle' }, subcategories: { type: 'idle' }, tried: false, creating: false, error: null,
    }
    this.editTarget = { ticket: t, original, locked }
    this.newTicket.edit = s
    this.changed()
    await this.wait(200)
    if (this.newTicket.edit !== s) return
    s.orgs = { type: 'loaded', value: NEW_TICKET_ORGS }
    s.callers = { type: 'loaded', value: CALLERS[org.id] ?? [] }
    s.services = { type: 'loaded', value: SERVICES }
    s.subcategories = { type: 'loaded', value: SUBCATEGORIES[original.serviceId ?? ''] ?? [] }
  }

  private newTicketSnapshot(which: 'sheet' | 'window' | 'edit'): SnapshotOf<'newTicket:sheet'> {
    const s = this.newTicket[which]
    if (!s) return { type: 'closed' }
    const target = which === 'edit' ? this.editTarget : null
    const changed = target !== null && JSON.stringify(this.draftOf(s)) !== JSON.stringify({ ...target.original, impact: target.original.impact ?? '1', urgency: target.original.urgency ?? '4' })
    const orgName = NEW_TICKET_ORGS.find((o) => o.id === s.orgId)?.name ?? null
    const missing = s.tried ? this.firstMissing(s) : null
    const orgs = valueOf(s.orgs)
    const callers = valueOf(s.callers)
    const services = valueOf(s.services)
    const subs = valueOf(s.subcategories)
    const mark = (empty: boolean) => ({ missing: s.tried && empty })
    return {
      type: 'open',
      subtitle: s.start.ciName ? `On ${s.start.ciName}` : null,
      ticketClass: s.ticketClass,
      organization: {
        items: pick(orgs ?? []), selectedId: s.orgId, placeholder: placeholderOf(s.orgs, 'Loading organizations…', 'No organizations found'),
        enabled: orgs !== null, search: { prompt: 'Search organizations', noMatch: 'No organization matches' },
      },
      caller: {
        items: s.orgId ? pick(callers ?? []) : [], selectedId: s.callerId,
        placeholder: s.orgId ? placeholderOf(s.callers, 'Loading people…', `Nobody in ${orgName ?? 'this organization'}`) : 'Choose an organization first',
        enabled: s.orgId !== null && callers !== null,
        search: { prompt: `Search people in ${orgName ?? 'the organization'}`, noMatch: `No caller in ${orgName ?? 'this organization'} matches` },
      },
      service: {
        items: s.orgId ? pick(services ?? []) : [], selectedId: s.serviceId,
        placeholder: s.orgId ? placeholderOf(s.services, 'Loading…', 'No services') : 'Choose an organization first',
        enabled: s.orgId !== null && services !== null, search: null,
      },
      subcategory: {
        items: s.serviceId ? pick(subs ?? []) : [], selectedId: s.subcategoryId,
        placeholder: s.serviceId ? placeholderOf(s.subcategories, 'Loading…', 'None for this service') : 'Choose a service first',
        enabled: s.serviceId !== null && subs !== null, search: null,
      },
      impact: { items: pick(IMPACTS), selectedId: s.impact, placeholder: 'Choose…', enabled: true, search: null },
      urgency: { items: pick(URGENCIES), selectedId: s.urgency, placeholder: 'Choose…', enabled: true, search: null },
      title: s.title,
      description: s.description,
      assignToMe: this.assignToMePref,
      required: {
        organization: mark(!s.orgId), caller: mark(!s.callerId), service: mark(!s.serviceId), subcategory: mark(!s.subcategoryId),
        title: mark(s.title.trim() === ''), description: mark(!target?.locked && s.description.trim() === ''),
      },
      missingText: missing ? `Fill in the ${missing} first.` : null,
      error: s.error,
      disabledText: this.canWrite ? null : 'Creating tickets is switched off in this build, so nothing is sent to iTop.',
      creating: s.creating,
      confirmEnabled: !s.creating && this.canWrite && (target === null || changed),
      showClearForm: target === null && this.newTicketDraft !== null,
      edit: target && {
        title: `Edit ${target.ticket.ref}`,
        loading: valueOf(s.orgs) === null,
        descriptionLocked: target.locked,
        descriptionNote: target.locked ? 'This description has images or formatting, so it can only be changed in iTop on the web.' : null,
        webUrl: `https://${HOST}/pages/UI.php?operation=details&class=${target.ticket.ticketClass}&id=${target.ticket.key}`,
        changed,
      },
    }
  }

  private async editIntent(intent: IntentOf<'newTicket:sheet'>): Promise<unknown> {
    const s = this.newTicket.edit
    const target = this.editTarget
    if (!s || !target) return null
    switch (intent.type) {
      case 'open':
      case 'setClass':
      case 'setAssignToMe':
      case 'clearForm':
        throw new BackendError({ type: 'invalidIntent', message: "Edit ticket can't change the type, assign, clear or open on its own." })
      case 'cancel':
      case 'dismiss':
        this.newTicket.edit = null
        this.editTarget = null
        return null
      case 'create': {
        s.tried = true
        s.error = null
        if (this.firstMissing(s) && !(target.locked && this.firstMissing(s) === 'description')) return null
        if (!this.canWrite) return null
        s.creating = true
        this.changed()
        try {
          await this.wait(400)
          const row = this.db.get(target.ticket.ref)!
          Object.assign(row, { title: s.title.trim(), lastUpdate: this.now() })
          this.newTicket.edit = null
          this.editTarget = null
          const list = this.lists.mine
          list.notice = { text: `${row.ref} is updated.`, isError: false }
          await this.refresh('mine')
          return null
        } finally {
          s.creating = false
        }
      }
      default:
        // Field changes behave as in New ticket.
        return this.fieldChange(s, intent)
    }
  }

  /** The field changes New ticket and Edit ticket share (with the org → service cascade). */
  private async fieldChange(s: NewTicketState, intent: IntentOf<'newTicket:sheet'>): Promise<unknown> {
    switch (intent.type) {
      case 'setOrganization':
        s.orgId = intent.id
        s.callerId = null
        s.serviceId = null
        s.subcategoryId = null
        s.callers = { type: 'idle' }
        s.services = { type: 'idle' }
        if (intent.id) await this.loadForOrg(s, intent.id)
        return null
      case 'setCaller':
        s.callerId = intent.id
        return null
      case 'setService':
        s.serviceId = intent.id
        s.subcategoryId = null
        s.subcategories = { type: 'idle' }
        if (intent.id) await this.loadNewSubcategories(s, intent.id)
        return null
      case 'setSubcategory':
        s.subcategoryId = intent.id
        return null
      case 'setImpact':
        s.impact = intent.id
        return null
      case 'setUrgency':
        s.urgency = intent.id
        return null
      case 'setTitle':
        s.title = intent.title
        return null
      case 'setDescription':
        s.description = intent.description
        return null
      default:
        return null
    }
  }

  private draftOf(s: NewTicketState): NewTicketDraft {
    return {
      ticketClass: s.ticketClass, orgId: s.orgId, callerId: s.callerId, serviceId: s.serviceId, subcategoryId: s.subcategoryId,
      impact: s.impact ?? '1', urgency: s.urgency ?? '4', title: s.title, description: s.description,
    }
  }

  private async newTicketIntent(which: 'sheet' | 'window' | 'edit', intent: IntentOf<'newTicket:sheet'>): Promise<unknown> {
    if (which === 'edit') return this.editIntent(intent)
    if (intent.type === 'open') {
      await this.openNewTicket(which, { orgId: null, orgName: null, ciId: null, ciName: null })
      return null
    }
    const s = this.newTicket[which]
    if (!s) return null
    switch (intent.type) {
      case 'setClass':
        s.ticketClass = intent.ticketClass
        return null
      case 'setOrganization':
      case 'setCaller':
      case 'setService':
      case 'setSubcategory':
      case 'setImpact':
      case 'setUrgency':
      case 'setTitle':
      case 'setDescription':
        return this.fieldChange(s, intent)
      case 'setAssignToMe':
        this.assignToMePref = intent.assignToMe
        return null
      case 'clearForm':
        this.newTicketDraft = null
        Object.assign(s, blankDraft(s.ticketClass), { callers: { type: 'idle' }, services: { type: 'idle' }, subcategories: { type: 'idle' }, tried: false, error: null })
        return null
      case 'cancel': {
        const d = this.draftOf(s)
        const empty = !d.orgId && d.title.trim() === '' && d.description.trim() === ''
        this.newTicketDraft = empty ? null : d
        this.newTicket[which] = null
        return null
      }
      case 'dismiss':
        this.newTicket[which] = null
        return null
      case 'create': {
        s.tried = true
        s.error = null
        if (this.firstMissing(s)) return null
        if (!this.canWrite) return null
        s.creating = true
        this.changed()
        try {
          await this.wait(500)
          if (/fail/i.test(s.title)) {
            s.error = `iTop didn't create the ticket. ${withPeriod(SERVER_ERROR('Error: servicesubcategory_id: not allowed for this request type'))}`
            return s.error
          }
          const n = ++this.lastTicketNumber
          const org = NEW_TICKET_ORGS.find((o) => o.id === s.orgId)!
          const created: MockTicket = {
            key: String(n), ticketClass: s.ticketClass, ref: `${s.ticketClass === 'Incident' ? 'I' : 'R'}-${String(n).padStart(6, '0')}`,
            title: s.title.trim(), status: 'new', priority: 4, callerName: (CALLERS[org.id] ?? []).find((c) => c.id === s.callerId)?.name ?? '',
            orgName: org.name, orgId: org.id, teamId: '0', teamName: '', agentId: '0', agentName: '', startDate: this.now(), lastUpdate: this.now(),
          }
          this.db.set(created.ref, created)
          this.newTicketDraft = null
          this.newTicket[which] = null
          this.selection = 'myTickets'
          const mine = this.lists.mine
          mine.query = created.ref
          mine.queryToken++
          this.setLookup(mine, { type: 'found', ticket: { ...created } })
          mine.selection = ticketId(created)
          mine.notice = { text: `${created.ref} is created.`, isError: false }
          this.syncDetails()
          if (this.assignToMePref) void this.assignNew(created)
          return null
        } finally {
          s.creating = false
        }
      }
    }
  }

  private async assignNew(t: MockTicket) {
    await this.assignToMe('mine', t)
    const notice = this.lists.mine.notice
    if (notice && !notice.isError) this.lists.mine.notice = { text: `${t.ref} is created and assigned to you.`, isError: false }
    else if (notice) this.lists.mine.notice = { text: `${t.ref} is created, but not assigned to you. ${notice.text}`, isError: true }
    this.changed()
  }

  // ===========================================================================================
  // timer
  // ===========================================================================================

  private canStart(t: MockTicket) {
    return t.status === 'assigned' && t.agentId === ME.contactId
  }

  private checkWebStopwatch(t: MockTicket) {
    if (!this.canStart(t) || this.timer.running?.ticketId === ticketId(t)) return
    if (t.ref === WEB_STOPWATCH_REF) this.timer.web[ticketId(t)] = this.webSince
  }

  private ticking() {
    if (this.timer.running && !this.tickTimer) {
      this.tickTimer = setInterval(() => this.changed(), 1000)
      ;(this.tickTimer as { unref?: () => void }).unref?.()
    } else if (!this.timer.running && this.tickTimer) {
      clearInterval(this.tickTimer)
      this.tickTimer = null
    }
  }

  private async startTimer(t: MockTicket) {
    const id = ticketId(t)
    if (!this.canStart(t) || this.timer.running?.ticketId === id || this.timer.saving) return
    if (!this.canWrite) {
      this.timer.notice = { text: 'Saving time to iTop is switched off in this build, so the timer can\'t start.', isError: true, canRetry: false, dismissLabel: 'Dismiss' }
      return
    }
    if (t.ref === WEB_STOPWATCH_REF) {
      this.timer.web[id] = this.webSince
      this.timer.notice = {
        text: `Your web stopwatch is already running on ${t.ref} since ${sinceText(this.webSince, this.now())}. Stop it in iTop, or keep using it there.`,
        isError: false, canRetry: false, dismissLabel: 'Dismiss',
      }
      return
    }
    delete this.timer.web[id]
    if (this.timer.running) await this.stopTimer(null)
    this.timer.running = { ticketId: id, ref: t.ref, title: t.title, start: this.now() }
    this.timer.notice = null
    this.ticking()
  }

  private async stopTimer(reason: string | null) {
    const r = this.timer.running
    if (!r) return
    const elapsed = Math.max(0, (this.now().getTime() - r.start.getTime()) / 1000)
    const capped = elapsed > TIMER_LIMIT
    const seconds = Math.floor(Math.min(elapsed, TIMER_LIMIT))
    this.timer.running = null
    this.ticking()
    const prefix = reason ? `${reason} ` : ''
    if (seconds < 60) {
      this.timer.notice = { text: `${prefix}The timer on ${r.ref} ran under a minute, so nothing was saved.`, isError: false, canRetry: false, dismissLabel: 'Dismiss' }
      return
    }
    await this.saveTime({ seconds, ref: r.ref, ticketId: r.ticketId }, prefix, capped)
  }

  private async saveTime(entry: { seconds: number; ref: string; ticketId: string }, prefix: string, capped: boolean) {
    this.timer.saving = true
    this.changed()
    try {
      await this.wait(400)
      if (this.options.timeFail) {
        this.timer.failed = entry
        this.timer.notice = {
          text: `${prefix}Couldn't save ${clock(entry.seconds)} to ${entry.ref}. ${withPeriod(SERVER_ERROR("Error: You don't have the Time Tracker profile"))}`,
          isError: true, canRetry: true, dismissLabel: 'Discard this time',
        }
        return
      }
      this.timer.failed = null
      this.timer.totals[entry.ticketId] = (this.timer.totals[entry.ticketId] ?? 0) + entry.seconds
      const cap = capped ? ` It stopped at ${clock(entry.seconds)}, like iTop's own stopwatch.` : ''
      this.timer.notice = { text: `${prefix}Saved ${clock(entry.seconds)} to ${entry.ref}.${cap}`, isError: false, canRetry: false, dismissLabel: 'Dismiss' }
    } finally {
      this.timer.saving = false
    }
  }

  private async myTicketsChanged() {
    const r = this.timer.running
    if (!r) return
    const t = this.lists.mine.tickets.find((x) => ticketId(x) === r.ticketId)
    if (t && this.canStart(t)) return
    await this.stopTimer(`${r.ref} left Assigned, so its timer stopped.`)
  }

  private timerSnapshot(): SnapshotOf<'timer'> {
    const r = this.timer.running
    const elapsed = r ? Math.max(0, Math.floor((this.now().getTime() - r.start.getTime()) / 1000)) : 0
    return {
      running: r
        ? { ticketId: r.ticketId, ref: r.ref, title: r.title, startedAt: r.start.toISOString(), elapsedSeconds: elapsed, counter: counter(elapsed), clock: clock(elapsed) }
        : null,
      saving: this.timer.saving,
      notice: this.timer.notice,
    }
  }

  private async timerIntent(intent: IntentOf<'timer'>): Promise<unknown> {
    switch (intent.type) {
      case 'start':
      case 'toggle': {
        const t = this.findTicket(intent.ticketId)
        if (!t) throw new BackendError({ type: 'invalidIntent', message: `No ticket ${intent.ticketId}` })
        if (intent.type === 'toggle' && this.timer.running?.ticketId === intent.ticketId) await this.stopTimer(null)
        else await this.startTimer(t)
        return null
      }
      case 'stop':
        await this.stopTimer(null)
        return null
      case 'retry':
        if (this.timer.failed) await this.saveTime(this.timer.failed, '', false)
        return null
      case 'clearNotice':
        this.timer.notice = null
        this.timer.failed = null
        return null
    }
  }

  // ===========================================================================================
  // cmdb
  // ===========================================================================================

  private ciId(ci: MockCI) {
    return `${ci.className}::${ci.key}`
  }

  private async cmdbSearch() {
    const c = this.cmdb
    const token = ++c.token
    const q = c.query.trim()
    if (q !== '' && c.scope) c.scope = null
    if (c.scope) {
      if (c.searched === `org:${c.scope.orgId}` && c.results.type === 'loaded') return
      c.results = { type: 'loading' }
      this.changed()
      await this.wait(200)
      if (token !== c.token || !c.scope) return
      c.results = { type: 'loaded', value: c.scope.orgId === '1' ? this.cmdbData.items : [] }
      c.searched = `org:${c.scope.orgId}`
      return
    }
    if (q.length < 2) {
      c.results = { type: 'idle' }
      c.searched = null
      return
    }
    if (q === c.searched && c.results.type === 'loaded') return
    c.results = { type: 'loading' }
    this.changed()
    await this.wait(300)
    if (token !== c.token) return
    if (this.options.listState === 'error') {
      c.results = { type: 'failed', message: NETWORK_ERROR }
      return
    }
    const items =
      this.options.listState === 'empty'
        ? []
        : this.cmdbData.items.filter((i) => i.name.toLowerCase().includes(q.toLowerCase())).sort((a, b) => natural(a.name, b.name))
    if (c.query.trim() !== q) return
    c.results = { type: 'loaded', value: items }
    c.searched = q
  }

  private async openCI(ci: MockCI) {
    this.cmdb.current = ci
    this.cmdb.selectedId = this.ciId(ci)
    await this.loadCIDetail()
  }

  private async loadCIDetail() {
    const c = this.cmdb
    const item = c.current
    if (!item) return
    c.detail = { type: 'loading' }
    this.changed()
    await this.wait(200)
    if (c.current !== item) return
    c.detail = {
      type: 'loaded',
      value: this.cmdbData.details[this.ciId(item)] ?? { fields: [], description: '', dependsOn: [], usedBy: [], openTickets: [] },
    }
  }

  private cmdbSnapshot(): SnapshotOf<'cmdb'> {
    const c = this.cmdb
    const q = c.query.trim()
    let results: SnapshotOf<'cmdb'>['results']
    switch (c.results.type) {
      case 'idle':
        results = { type: 'idle', text: 'Search by name, for example a server or application.' }
        break
      case 'loading':
        results = { type: 'searching' }
        break
      case 'failed':
        results = { type: 'failed', text: `Couldn't search iTop. ${c.results.message}` }
        break
      case 'loaded': {
        const items = c.results.value
        results =
          items.length === 0
            ? { type: 'empty', text: `No config items match “${q}”.` }
            : {
                type: 'loaded',
                header: items.length === 1 ? '1 result' : items.length >= 50 ? `First ${items.length} results` : `${items.length} results`,
                items: items.map((i) => ({
                  id: this.ciId(i), key: i.key, className: i.className, name: i.name, subtitle: [classLabel(i.className), i.orgName].filter(Boolean).join(' · '),
                })),
              }
      }
    }
    const item = c.current
    const chip = (ci: MockCI): CIChip => ({ key: ci.key, className: ci.className, name: ci.name, help: `${classLabel(ci.className)}: open ${ci.name}` })
    const detail = item
      ? (() => {
          const d = c.detail
          const status = d.type === 'loaded' ? d.value.fields.find((f) => f.label === 'Status')?.value : undefined
          return {
            id: this.ciId(item), key: item.key, className: item.className, name: item.name, orgName: item.orgName,
            headerLine: [classLabel(item.className), status].filter(Boolean).join(' · '),
            copied: c.copied,
            webUrl: this.webUrl(item.className, item.key),
            body:
              d.type === 'loaded'
                ? {
                    type: 'loaded' as const,
                    fields: d.value.fields,
                    description: d.value.description || null,
                    dependsOn: d.value.dependsOn.map(chip),
                    usedBy: d.value.usedBy.map(chip),
                    usedBySentence:
                      d.value.usedBy.length === 0
                        ? `Nothing in the CMDB depends on ${item.name}.`
                        : `If ${item.name} fails, ${listJoin(d.value.usedBy.map((u) => u.name))} ${d.value.usedBy.length === 1 ? 'is' : 'are'} affected.`,
                    openTickets: d.value.openTickets.map((t) => ({
                      ticketId: ticketId(t), ref: t.ref, title: t.title, needsYou: this.needsYou(t),
                      statusLine: `${statusLabel(t.status)} · ${isUnassigned(t) ? 'Unassigned' : t.agentId === ME.contactId ? 'You' : t.agentName}`,
                    })),
                    openTicketsEmptyText: d.value.openTickets.length === 0 ? `No open tickets on ${item.name}.` : null,
                  }
                : d.type === 'failed'
                  ? { type: 'failed' as const, text: `Couldn't load this config item. ${d.message}` }
                  : { type: 'loading' as const, text: `Loading ${item.name} from iTop` },
          }
        })()
      : null
    return { query: c.query, scope: c.scope, results, selectedId: c.selectedId, current: detail, placeholder: 'Select a config item to see its details.' }
  }

  private async cmdbIntent(intent: IntentOf<'cmdb'>): Promise<unknown> {
    const c = this.cmdb
    switch (intent.type) {
      case 'setQuery':
        c.query = intent.query
        void this.cmdbSearch().then(() => this.changed())
        return null
      case 'clearScope':
        c.scope = null
        c.results = { type: 'idle' }
        c.searched = null
        return null
      case 'select': {
        c.selectedId = intent.id
        const item = c.results.type === 'loaded' ? c.results.value.find((i) => this.ciId(i) === intent.id) : undefined
        if (item && item !== c.current) await this.openCI(item)
        return null
      }
      case 'openCI':
        await this.openCI({ key: intent.key, className: intent.className, name: intent.name, orgName: '' })
        return null
      case 'retryDetail':
        await this.loadCIDetail()
        return null
      case 'copyName':
        if (!c.current) return null
        try {
          await navigator.clipboard?.writeText(c.current.name)
        } catch {
          // No clipboard in tests.
        }
        c.copied = true
        setTimeout(() => {
          c.copied = false
          this.changed()
        }, 2000)
        return null
    }
  }

  // ===========================================================================================
  // orgs
  // ===========================================================================================

  private teamSourceTickets(includeAllScope: boolean): MockTicket[] {
    const now = this.now()
    return [...this.db.values()]
      .filter((t) => t.ref !== 'R-000512')
      .filter((t) => (includeAllScope ? !['resolved', 'closed'].includes(t.status) : this.includeClosed || !['resolved', 'closed'].includes(t.status)))
      .filter((t) => includeAllScope || this.range === 0 || (t.startDate ?? now).getTime() >= now.getTime() - this.range * 86_400_000)
  }

  private async loadOrgList() {
    if (this.orgs.list.type === 'loaded') return
    this.orgs.list = { type: 'loading' }
    this.changed()
    await this.wait()
    if (this.options.listState === 'error') {
      this.orgs.list = { type: 'failed', message: NETWORK_ERROR }
      return
    }
    const open = this.teamSourceTickets(true)
    this.orgs.list = { type: 'loaded', value: ORGS.map((o) => ({ ...o, openCount: open.filter((t) => t.orgName === o.name).length })) }
  }

  private async loadOrgSelected() {
    const id = this.orgs.selectedId
    if (!id) return
    this.orgs.tickets = { type: 'loading' }
    this.orgs.counts = null
    this.orgs.page = 1
    this.changed()
    await this.wait()
    if (this.orgs.selectedId !== id) return
    const org = ORGS.find((o) => o.id === id)
    this.orgs.tickets = { type: 'loaded', value: this.teamSourceTickets(false).filter((t) => t.orgName === org?.name) }
    this.orgs.counts = ORG_COUNTS[id] ?? null
  }

  private orgsSnapshot(): SnapshotOf<'orgs'> {
    const o = this.orgs
    const q = o.query.trim().toLowerCase()
    const all = valueOf(o.list) ?? []
    const visible = q === '' ? all : all.filter((x) => `${x.name} ${x.code}`.toLowerCase().includes(q))
    const list: SnapshotOf<'orgs'>['list'] =
      o.list.type === 'failed'
        ? { type: 'failed', text: `Couldn't load organizations. ${o.list.message}` }
        : o.list.type !== 'loaded'
          ? { type: 'loading' }
          : visible.length === 0
            ? { type: 'empty', text: o.query === '' ? "Your iTop account can't see any organizations." : `No organization matches “${o.query.trim()}”.` }
            : { type: 'loaded', orgs: visible }
    const org = all.find((x) => x.id === o.selectedId)
    let tickets: TableState = { type: 'loading', text: `Loading ${org?.name ?? ''}'s tickets` }
    if (org && o.tickets.type === 'failed') tickets = { type: 'failed', text: `Couldn't load the tickets. ${o.tickets.message}`, canRetry: true }
    if (org && o.tickets.type === 'loaded') {
      tickets =
        o.tickets.value.length === 0
          ? { type: 'empty', text: this.isDefaultScope() ? `${org.name} has no open tickets.` : `${org.name} has no tickets in this range.` }
          : this.table(o.tickets.value, 'assignee', o.page)
    }
    return {
      query: o.query,
      list,
      selectedId: o.selectedId,
      detail: org
        ? {
            id: org.id, name: org.name, code: org.code,
            headerLine: org.code ? `Organization · ${org.code}` : 'Organization',
            peopleLabel: o.counts ? `People ${o.counts.people}` : 'People',
            configItemsLabel: o.counts ? `Config items ${o.counts.configItems}` : 'Config items',
            ticketsHeading: this.includeClosed ? 'Tickets' : 'Open tickets',
            tickets,
          }
        : null,
      placeholder: 'Select an organization to see its open tickets.',
    }
  }

  private async orgsIntent(intent: IntentOf<'orgs'>): Promise<unknown> {
    switch (intent.type) {
      case 'setQuery':
        this.orgs.query = intent.query
        return null
      case 'select':
        this.orgs.selectedId = intent.id
        await this.loadOrgSelected()
        return null
      case 'retryList':
        this.orgs.list = { type: 'idle' }
        await this.loadOrgList()
        return null
      case 'retryTickets':
        await this.loadOrgSelected()
        return null
      case 'setTicketPage':
        this.orgs.page = intent.page
        return null
    }
  }

  // ===========================================================================================
  // people
  // ===========================================================================================

  private async peopleSearch() {
    const p = this.people
    const token = ++p.token
    const q = p.query.trim()
    if (q !== '' && p.scope) p.scope = null
    if (p.scope) {
      if (p.searched === `org:${p.scope.orgId}` && p.results.type === 'loaded') return
      p.results = { type: 'loading' }
      this.changed()
      await this.wait(200)
      if (token !== p.token || !p.scope) return
      const found = PEOPLE.filter((x) => x.orgId === p.scope!.orgId)
      p.results = { type: 'loaded', value: found }
      p.total = found.length
      p.searched = `org:${p.scope.orgId}`
      return
    }
    if (q.length < 2) {
      p.results = { type: 'idle' }
      p.searched = null
      return
    }
    if (q === p.searched && p.results.type === 'loaded') return
    p.results = { type: 'loading' }
    this.changed()
    await this.wait(300)
    if (token !== p.token) return
    const words = q.toLowerCase().split(' ').filter(Boolean)
    const found = PEOPLE.filter((x) => words.every((w) => x.displayName.toLowerCase().includes(w))).sort((a, b) => natural(a.displayName, b.displayName))
    if (p.query.trim() !== q) return
    p.results = { type: 'loaded', value: found }
    p.total = found.length
    p.searched = q
  }

  private async loadPersonSelected(keepTab: boolean) {
    const p = this.people
    const person = (valueOf(p.results) ?? []).find((x) => x.id === p.selectedId)
    if (!person) return
    if (!keepTab) p.tab = person.teams.length > 0 ? 'assigned' : 'reported'
    p.assigned = { type: 'loading' }
    p.reported = { type: 'loading' }
    p.page = 1
    this.changed()
    await this.wait()
    if (p.selectedId !== person.id) return
    const tickets = this.teamSourceTickets(false)
    p.assigned = { type: 'loaded', value: tickets.filter((t) => t.agentId === person.id) }
    p.reported = { type: 'loaded', value: tickets.filter((t) => t.callerName === CALLER_OF[person.id]) }
  }

  private peopleSnapshot(): SnapshotOf<'people'> {
    const p = this.people
    let results: SnapshotOf<'people'>['results']
    switch (p.results.type) {
      case 'idle':
        results = { type: 'idle', text: 'Search by name, for example a caller or an agent.' }
        break
      case 'loading':
        results = { type: 'searching' }
        break
      case 'failed':
        results = { type: 'failed', text: `Couldn't search iTop. ${p.results.message}` }
        break
      case 'loaded': {
        const people = p.results.value
        results =
          people.length === 0
            ? { type: 'empty', text: p.scope ? 'Nobody in this organization.' : `Nobody matches “${p.query.trim()}”.` }
            : {
                type: 'loaded',
                banner:
                  p.total > people.length
                    ? p.scope
                      ? `Showing ${people.length} of ${p.total} people. Search by name to find someone.`
                      : `Showing ${people.length} of ${p.total} people. Type more of the name to narrow it down.`
                    : null,
                people: people.map((x) => ({ id: x.id, displayName: x.displayName, initials: initialsOf(x.displayName), subtitle: this.personSubtitle(x) })),
              }
      }
    }
    const person = (valueOf(p.results) ?? []).find((x) => x.id === p.selectedId)
    const current = p.tab === 'assigned' ? p.assigned : p.reported
    const count = (l: Loadable<MockTicket[]>) => (l.type === 'loaded' ? ` ${l.value.length}` : '')
    let tickets: TableState = { type: 'loading', text: 'Loading tickets' }
    if (person && current.type === 'failed') tickets = { type: 'failed', text: `Couldn't load the tickets. ${current.message}`, canRetry: false }
    if (person && current.type === 'loaded') {
      const kind = this.isDefaultScope() ? 'open tickets' : 'tickets in this range'
      tickets =
        current.value.length === 0
          ? { type: 'empty', text: p.tab === 'assigned' ? `No ${kind} assigned to ${person.displayName}.` : `No ${kind} reported by ${person.displayName}.` }
          : this.table(current.value, p.tab === 'assigned' ? 'caller' : 'assignee', p.page)
    }
    return {
      query: p.query,
      scope: p.scope,
      results,
      selectedId: p.selectedId,
      detail: person
        ? {
            id: person.id, displayName: person.displayName, initials: initialsOf(person.displayName),
            kindLine: [person.teams.length > 0 ? 'Agent' : 'Caller', person.function].filter(Boolean).join(' · '),
            organization: person.orgName ? { orgId: person.orgId, orgName: person.orgName } : null,
            email: person.email || null,
            phone: person.phone || null,
            teams: person.teams.length > 0 ? person.teams.map((t) => t.name).join(', ') : null,
            tab: p.tab,
            assignedLabel: `Assigned to them${count(p.assigned)}`,
            reportedLabel: `Reported by them${count(p.reported)}`,
            tickets,
          }
        : null,
      placeholder: 'Select a person to see their details and tickets.',
    }
  }

  private personSubtitle(x: MockPerson) {
    return x.teams.length > 0 ? `Agent · ${x.teams.map((t) => t.name).join(', ')}` : `Caller${x.orgName ? ` · ${x.orgName}` : ''}`
  }

  private async peopleIntent(intent: IntentOf<'people'>): Promise<unknown> {
    const p = this.people
    switch (intent.type) {
      case 'setQuery':
        p.query = intent.query
        void this.peopleSearch().then(() => this.changed())
        return null
      case 'clearScope':
        p.scope = null
        p.results = { type: 'idle' }
        p.searched = null
        return null
      case 'select':
        p.selectedId = intent.id
        await this.loadPersonSelected(false)
        return null
      case 'setTab':
        p.tab = intent.tab
        p.page = 1
        return null
      case 'setTicketPage':
        p.page = intent.page
        return null
    }
  }

  // ===========================================================================================
  // menubar
  // ===========================================================================================

  private menuSnapshot(): SnapshotOf<'menubar'> {
    const mine = this.lists.mine
    const running = this.timer.running
    const needCount = this.needCount()
    const elapsed = running ? Math.max(0, (this.now().getTime() - running.start.getTime()) / 1000) : 0
    const titleParts = [...(running ? [counter(elapsed)] : []), ...(needCount > 0 ? [String(needCount)] : [])]
    const a11y =
      `iTop Wrap: ${needCount === 0 ? 'nothing needs you' : needCount === 1 ? '1 ticket needs you' : `${needCount} tickets need you`}` +
      (running ? `, timer ${clock(elapsed)} on ${running.ref}` : '')
    const order = ['escalatedTTR', 'escalatedTTO', 'assigned', 'pending']
    const sections = groups(searchFilter(mine.tickets.filter((t) => !['resolved', 'closed'].includes(t.status)), this.menuQuery), 'mine', this.sort)
      .filter((s) => order.includes(s.group))
      .sort((a, b) => order.indexOf(a.group) - order.indexOf(b.group))
    const row = (t: MockTicket): MenuRow => {
      const id = ticketId(t)
      const isRunning = running?.ticketId === id
      const spent = this.timer.totals[id] ?? 0
      return {
        ticketId: id, ref: t.ref, title: t.title, needsYou: this.needsYou(t), isRunning,
        canResolve: actionsFor(t.status).includes('resolve'),
        right: isRunning ? { type: 'running', counter: counter(elapsed) } : spent > 0 ? { type: 'spent', text: total(spent) } : { type: 'none' },
        button: isRunning
          ? { type: 'stop' }
          : this.canStart(t) && !this.timer.web[id]
            ? { type: 'start', enabled: !this.timer.saving, help: running ? `Stops the timer on ${running.ref}, saves its time, and starts one here` : 'Start timer' }
            : { type: 'slot' },
      }
    }
    const pinned = running ? sections.flatMap((s) => s.tickets).find((t) => ticketId(t) === running.ticketId) : undefined
    const rest = pinned
      ? sections.map((s) => ({ ...s, tickets: s.tickets.filter((t) => t !== pinned) })).filter((s) => s.tickets.length > 0)
      : sections
    const rowCount = sections.reduce((n, s) => n + s.tickets.length, 0)
    let content: SnapshotOf<'menubar'>['content']
    if (mine.phase.type === 'loading') content = { type: 'loading' }
    else if (mine.phase.type === 'failed') content = { type: 'failed', text: `Couldn't load your tickets. ${mine.phase.message}` }
    else if (sections.length === 0) {
      content = { type: 'empty', text: this.menuQuery === '' ? 'Nothing waiting on you. New assignments show up here.' : `No tickets of yours match “${this.menuQuery}”.` }
    } else {
      content = {
        type: 'list',
        running: pinned ? row(pinned) : null,
        sections: rest.map((s) => ({ group: s.group, title: `${GROUP_TITLES[s.group]}  ${s.tickets.length}`, rows: s.tickets.map(row) })),
        height: Math.min(420, (rest.length + (pinned ? 1 : 0)) * 30 + rowCount * 48 + 6),
      }
    }
    return {
      trayTitle: titleParts.join('  '),
      trayAccessibilityLabel: a11y,
      query: this.menuQuery,
      content,
      refreshing: mine.refreshing,
      updatedText: mine.updatedAt ? `Updated ${relativeNamed(mine.updatedAt, this.now())}` : null,
    }
  }

  private async menuIntent(intent: IntentOf<'menubar'>): Promise<unknown> {
    switch (intent.type) {
      case 'setQuery':
        this.menuQuery = intent.query
        return null
      case 'openTicket':
        this.selection = 'myTickets'
        this.setQuery('mine', '')
        this.lists.mine.selection = intent.ticketId
        this.syncDetails()
        return null
      case 'resolve':
        // Like New ticket: only the Resolve window, the main window untouched.
        await this.resolveIntent('window', { type: 'open', ticketId: intent.ticketId })
        return null
      case 'openMain':
        return null
      case 'newTicket':
        await this.openNewTicket('window', { orgId: null, orgName: null, ciId: null, ciName: null })
        return null
      case 'refresh':
        await this.refresh('mine')
        return null
      case 'close':
        return null
    }
  }

  // ===========================================================================================
  // update (made up: the "latest release" is always 2.1.0, nothing is fetched)
  // ===========================================================================================

  private update = { checking: false, latest: null as string | null, dismissed: null as string | null, autoCheck: true, dialog: null as UpdateDialog | null }

  private updateSnapshot(): SnapshotOf<'update'> {
    const u = this.update
    const newer = u.latest !== null && u.latest !== MOCK_CURRENT ? u.latest : null
    return {
      current: MOCK_CURRENT,
      checking: u.checking,
      autoCheck: u.autoCheck,
      notice: newer && newer !== u.dismissed ? { version: newer, text: `iTop Wrap ${newer} is available` } : null,
      dialog: u.dialog,
    }
  }

  private async updateIntent(intent: IntentOf<'update'>): Promise<unknown> {
    const u = this.update
    switch (intent.type) {
      case 'check':
        u.checking = true
        u.dialog = null
        this.changed()
        await this.wait(400)
        u.checking = false
        u.latest = MOCK_LATEST
        u.dialog = {
          kind: 'available',
          title: `iTop Wrap ${MOCK_LATEST} is available`,
          message: `You have ${MOCK_CURRENT}. Download it from GitHub, then drag it to Applications to replace this version.`,
        }
        return null
      case 'closeDialog':
      case 'download':
        // The mock opens nothing.
        u.dialog = null
        return null
      case 'dismiss':
        u.dismissed = u.latest
        return null
      case 'setAutoCheck':
        u.autoCheck = intent.on
        return null
    }
  }

  // ===========================================================================================
  // settings (the mock has no Dock or menu bar: it only keeps the choice)
  // ===========================================================================================

  private icons = { dock: true, menuBar: true }

  private settingsSnapshot(): SnapshotOf<'settings'> {
    const { dock, menuBar } = this.icons
    return { showInDock: dock, showMenuBarIcon: menuBar, canHideDock: !dock || menuBar, canHideMenuBarIcon: !menuBar || dock }
  }

  private async settingsIntent(intent: IntentOf<'settings'>): Promise<unknown> {
    const next = { ...this.icons }
    switch (intent.type) {
      case 'open':
        return null
      case 'setShowInDock':
        next.dock = intent.on
        break
      case 'setShowMenuBarIcon':
        next.menuBar = intent.on
        break
    }
    // Both off would leave no way back to the app.
    if (next.dock || next.menuBar) this.icons = next
    return null
  }
}
