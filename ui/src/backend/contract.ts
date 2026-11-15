/**
 * The contract between the Rust backend (src-tauri) and the React UI.
 *
 * docs/rewrite/contract.md is the prose version: where every field comes from in the Swift app,
 * the lifetimes, the native side and the open points. This file is the shape.
 *
 * Rules for every type here (so serde can produce them 1:1):
 * - Field names are camelCase (`#[serde(rename_all = "camelCase")]`).
 * - Tagged unions use a `type` field (`#[serde(tag = "type", rename_all = "camelCase")]`), struct or
 *   unit variants only. String unions are plain unit enums.
 * - Optional is `T | null` (`Option<T>`); there is never an `undefined` or missing field.
 * - Dates are RFC 3339 strings. Display strings ("2h", "Today 09:12", "1h 20m") are made in Rust.
 * - Intents that return nothing resolve to `null` (Rust `()`).
 */

// ---------------------------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------------------------

/** RFC 3339 date-time, e.g. "2026-11-16T09:12:00+01:00". */
export type Rfc3339 = string

/** The two ticket lists. Each has its own list, detail and sheet models. */
export type ListScope = 'mine' | 'team'

/** Sidebar sections, in sidebar order (⌘1…⌘5). */
export type SidebarItem = 'myTickets' | 'teamTickets' | 'configItems' | 'organizations' | 'people'

export type TicketClass = 'UserRequest' | 'Incident'

/** Status groups (K/TicketQueries.swift:119). */
export type TicketGroup =
  | 'escalatedTTR'
  | 'escalatedTTO'
  | 'new'
  | 'assigned'
  | 'pending'
  | 'other'
  | 'resolved'
  | 'closed'

export type TicketSort = 'priority' | 'lastUpdate' | 'newest' | 'oldest'

/** "Show tickets opened": 0 = any time. */
export type RangeDays = 0 | 7 | 30 | 90

export type LogTab = 'public' | 'private'

export type PeopleTab = 'assigned' | 'reported'

/** One line of feedback: an action's result, or why it failed (warning colour when isError). */
export interface Notice {
  text: string
  isError: boolean
}

// ---------------------------------------------------------------------------------------------
// Shared records
// ---------------------------------------------------------------------------------------------

/** One ticket as iTop gives it, plus the per-user facts every view needs. */
export interface Ticket {
  /** "<Class>::<key>", e.g. "UserRequest::412". */
  id: string
  key: string
  ticketClass: TicketClass
  ref: string
  title: string
  /** iTop status code: "assigned", "escalated_ttr", … */
  status: string
  /** "Escalated TTR", "Waiting for approval", "Assigned" (W/TicketListView.swift:386). */
  statusLabel: string
  /** 1…4, missing = 4. */
  priority: number
  callerName: string
  orgName: string
  /** "0" when empty. */
  orgId: string
  teamId: string
  teamName: string
  agentId: string
  agentName: string
  startDate: Rfc3339 | null
  lastUpdate: Rfc3339 | null
  isUnassigned: boolean
  /** Assigned to the signed-in user. */
  isMine: boolean
  needsYou: boolean
}

/** Who has a ticket, as the detail strip and the org/person tables draw it. */
export type AgentChip =
  | { type: 'unassigned' }
  /** `name` is "You" for your own; `initials` are yours then, else the agent's. */
  | { type: 'person'; initials: string; name: string }

/** The agent column of a list row (W/TicketListView.swift:357). */
export type RowAgent =
  | { type: 'hidden' }
  /** Dashed "Assign to me" pill; dispatch `rowAssignToMe`. */
  | { type: 'assignToMe' }
  | { type: 'unassigned' }
  | { type: 'you' }
  | { type: 'name'; name: string }

/** The right side of a list row's third line. */
export type RowTime =
  | { type: 'none' }
  /** The live h:mm comes from the `timer` model (`running.clock`). */
  | { type: 'running' }
  /** Total time saved, e.g. "1h 20m". */
  | { type: 'spent'; text: string }

/** One row of My tickets / Team tickets. */
export interface TicketRow {
  ticket: Ticket
  /** "now", "40m", "2h", "9d"; null without a start date. */
  age: string | null
  /** Tooltip of the age: "Opened 16 Nov 2026 at 09:12". */
  openedTooltip: string | null
  /** P1 = red "!", P2 = grey dot, otherwise nothing. */
  priorityMark: 'p1' | 'p2' | null
  agent: RowAgent
  time: RowTime
}

export interface TicketSection {
  group: TicketGroup
  title: string
  /** The whole group's count, not just this page's share. */
  total: number
  rows: TicketRow[]
}

/** Under a paged list (W/ListOptions.swift:156). */
export type Pager =
  | { type: 'none' }
  /** One page: "1 ticket" / "12 tickets". */
  | { type: 'count'; text: string }
  /** More pages: ‹ "21–40 of 556" ›. */
  | { type: 'pages'; page: number; pageCount: number; label: string; canPrevious: boolean; canNext: boolean }

/** A piece of rich text (K/RichText.swift). */
export type RichRun =
  /** `link` is an http, https or mailto URL to open with `native.openUrl`. */
  | { type: 'text'; text: string; link: string | null }
  /** A small outside image (signature icon): a grey photo symbol, never loaded. */
  | { type: 'icon' }
  /** A larger outside image: chip "[photo] <host> ↗" that opens `url` in the browser. */
  | { type: 'outsideImage'; host: string; url: string }

export type RichBlock =
  | { type: 'text'; runs: RichRun[] }
  /**
   * An image to show. `src` is either an `itop://localhost/inline-image/<id>` asset URL (served by
   * Rust through REST) or a `data:image/...` URL. On load failure show "Couldn't load this image."
   * and, when `ticketWebUrl` is set, the "Open in iTop on the web" link.
   */
  | { type: 'image'; src: string; ticketWebUrl: string | null }

/** Parsed HTML. Never empty: when the parse yields nothing, Rust sends one text block with the plain text. */
export interface RichContent {
  blocks: RichBlock[]
}

export interface LogEntryView {
  id: number
  /** "<author> · <date>" with empty parts left out. */
  header: string
  isPrivate: boolean
  content: RichContent
}

export interface LinkedCI {
  key: string
  className: string
  name: string
}

/** An item in a dropdown (teams, agents, services, orgs, callers, codes, impact, urgency). */
export interface PickItem {
  id: string
  label: string
}

/** One picker (W/DropdownField.swift). Filtering and keyboard handling are the UI component's job. */
export interface Dropdown {
  items: PickItem[]
  selectedId: string | null
  /** Shown when nothing is selected: "Loading teams…", "Choose a team first", "Couldn't load: …", "Choose…". */
  placeholder: string
  enabled: boolean
  /** Searchable pickers (Organization, Caller) only. */
  search: { prompt: string; noMatch: string } | null
}

/** A required field's "Required" label: red once a confirm was tried with it empty. */
export interface RequiredMark {
  missing: boolean
}

/** A row of the org / person ticket tables (W/OrgsView.swift:7). */
export interface TableRow {
  ticketId: string
  ref: string
  title: string
  needsYou: boolean
  /** Only P1 is marked in tables. */
  p1: boolean
  callerName: string
  assignee: AgentChip
}

export interface TableSection {
  group: TicketGroup
  title: string
  total: number
  rows: TableRow[]
}

export interface TicketTable {
  /** "assignee": Ref / Title / Caller / Assignee. "caller": Ref / Title / Caller (caller last). */
  lastColumn: 'assignee' | 'caller'
  sections: TableSection[]
  pager: Pager
}

/** The ticket area of an org or a person. */
export type TableState =
  | { type: 'loading'; text: string }
  | { type: 'failed'; text: string; canRetry: boolean }
  | { type: 'empty'; text: string }
  | { type: 'loaded'; table: TicketTable }

/** A ticket as an open-ticket row under a config item. */
export interface CITicketRow {
  ticketId: string
  ref: string
  title: string
  needsYou: boolean
  /** "Assigned · You", "New · Unassigned". */
  statusLine: string
}

/** What `shell.openTicket` needs to find a ticket that came from a CI, an org or a person. */
export interface TicketLink {
  ticketId: string
  ref: string
}

/** An image handed to Rust from paste, drop or the picker; Rust validates it (LogImage rules). */
export interface ImageIntakeItem {
  /** The file's name; "" for raw clipboard image data (Rust names it "Pasted image.png" / ".tiff"). */
  filename: string
  /** Base64 of the file's bytes; null when the file was over `maxIntakeFileBytes` and not read. */
  dataBase64: string | null
  /** The file's size in bytes as the OS reported it. */
  size: number
}

export interface DraftImage {
  id: string
  filename: string
  /** File-style byte count: "1.2 MB". */
  sizeText: string
  /** Asset URL (`itop://localhost/draft-image/<id>`) or data URL for the 56 pt thumbnail. */
  thumbnailSrc: string
}

// ---------------------------------------------------------------------------------------------
// session
// ---------------------------------------------------------------------------------------------

export type LoginMessage =
  | { type: 'missingFields'; text: string }
  /** Bordered box: `bold` in warning colour, then `rest`. */
  | { type: 'restRefused'; bold: string; rest: string }
  | { type: 'error'; text: string }
  | { type: 'notice'; text: string }

export type LoginField = 'server' | 'user' | 'password'

export interface LoginForm {
  server: string
  user: string
  /** Server text starts with http:// (show the unencrypted warning). */
  httpWarning: boolean
  signingIn: boolean
  message: LoginMessage | null
  initialFocus: LoginField
}

export type SessionSnapshot =
  | { type: 'restoring' }
  | { type: 'signedOut'; form: LoginForm }
  | { type: 'unreachable'; message: string }
  | { type: 'signedIn'; displayName: string; host: string }

export type SessionIntent =
  | { type: 'setServer'; server: string }
  | { type: 'setUser'; user: string }
  | { type: 'signIn'; password: string }
  | { type: 'retryRestore' }
  | { type: 'signInWithAnotherAccount' }
  | { type: 'signOut' }

export interface SignInResult {
  ok: boolean
  /** Where focus goes after a failed submit (missing field, or Password on invalid credentials). */
  focus: 'user' | 'password' | null
}

export interface SessionResults {
  setServer: null
  setUser: null
  signIn: SignInResult
  retryRestore: null
  signInWithAnotherAccount: null
  signOut: null
}

// ---------------------------------------------------------------------------------------------
// shell (sidebar, account, navigation between sections)
// ---------------------------------------------------------------------------------------------

export interface SidebarRow {
  item: SidebarItem
  title: string
  /** Teal count on My tickets when > 0; null otherwise. */
  count: number | null
}

export interface ShellSnapshot {
  selection: SidebarItem
  sidebar: SidebarRow[]
  /** The content column's title: the selected section's title. */
  contentTitle: string
  needCount: number
  account: {
    initials: string
    displayName: string
    /** Server host, or "iTop". */
    host: string
  }
  signOutConfirm: { title: string; message: string }
}

export type ShellIntent =
  | { type: 'select'; item: SidebarItem }
  /** From a CI, org or person ticket row (W/ShellView.swift:120). */
  | { type: 'openTicket'; ticket: TicketLink }
  /** "See all of its tickets", a person's organization (W/ShellView.swift:107). */
  | { type: 'openOrg'; orgId: string; orgName: string }
  /** A ticket's linked CI (W/ShellView.swift:114). */
  | { type: 'openCI'; ci: LinkedCI }
  | { type: 'openOrgPeople'; orgId: string; orgName: string }
  | { type: 'openOrgConfigItems'; orgId: string; orgName: string }
  /** Opens `newTicket:sheet`. */
  | { type: 'newTicket' }
  | { type: 'newTicketForOrg'; orgId: string; orgName: string }
  | { type: 'newTicketForCI'; key: string; name: string; orgName: string }

export interface ShellResults {
  select: null
  openTicket: null
  openOrg: null
  openCI: null
  openOrgPeople: null
  openOrgConfigItems: null
  newTicket: null
  newTicketForOrg: null
  newTicketForCI: null
}

// ---------------------------------------------------------------------------------------------
// options (the shared View menu)
// ---------------------------------------------------------------------------------------------

export interface Choice<T> {
  value: T
  title: string
  selected: boolean
}

export interface OptionsSnapshot {
  range: RangeDays
  includeClosed: boolean
  sort: TicketSort
  /** null = All. */
  pageSize: number | null
  isDefault: boolean
  /** "All tickets, any time · 50 per page"; null when default. */
  summary: string | null
  /** The View button's tooltip. */
  help: string
  /** The View button's accessibility label. */
  accessibilityLabel: string
  rangeChoices: Choice<RangeDays>[]
  sortChoices: Choice<TicketSort>[]
  pageSizeChoices: Choice<number | null>[]
}

export type OptionsIntent =
  | { type: 'setRange'; range: RangeDays }
  | { type: 'setIncludeClosed'; includeClosed: boolean }
  | { type: 'setSort'; sort: TicketSort }
  | { type: 'setPageSize'; pageSize: number | null }
  | { type: 'reset' }

export interface OptionsResults {
  setRange: null
  setIncludeClosed: null
  setSort: null
  setPageSize: null
  reset: null
}

// ---------------------------------------------------------------------------------------------
// list:mine, list:team
// ---------------------------------------------------------------------------------------------

export interface OrgFilter {
  /** Shown only on My tickets, with > 1 org or a filter on. */
  visible: boolean
  selected: string | null
  /** The chosen org, or "All organizations". */
  label: string
  help: string
  accessibilityLabel: string
  choices: { name: string; selected: boolean }[]
}

export type LookupBar =
  | { type: 'searching'; text: string }
  | { type: 'missing'; text: string }
  | { type: 'failed'; text: string }

export interface ListBody {
  /** "None of your tickets are for <org>." with the optional "See all of its tickets" link. */
  orgNoMatch: { text: string; seeAll: { orgId: string; orgName: string } | null } | null
  /** "Couldn't refresh: <error>". */
  refreshError: string | null
  lookupBar: LookupBar | null
  /** The timer notice and the list notice show only while the lookup is idle or found. */
  showNotices: boolean
  notice: Notice | null
  /** The found ticket ("Search result: not in your queue") or "<Status> just now". */
  extraSection: { title: string; row: TicketRow } | null
  /** "No tickets in your list match “…”." */
  noMatchText: string | null
  /** The current page's sections. */
  sections: TicketSection[]
}

export type ListContent =
  | { type: 'loading' }
  | { type: 'failed'; message: string }
  /** No teams, or an empty list (not searching): the message, no Retry. */
  | { type: 'message'; text: string }
  | { type: 'list'; body: ListBody }

export interface ListSnapshot {
  scope: ListScope
  query: string
  selection: string | null
  content: ListContent
  /** Org filter + View menu above the list. */
  showControls: boolean
  orgFilter: OrgFilter
  /** Only when loaded. */
  pager: Pager
  /** Refresh disabled while true. */
  refreshing: boolean
  /** Mini spinner in the footer (refreshing after the first load). */
  footerSpinner: boolean
  /** "Updated just now", "Updated 3 min ago", "Loading", "Not updated yet" (re-made every 30 s). */
  updatedText: string
  needCount: number
  /** Ticket id an assign/lifecycle change runs on. */
  working: string | null
}

export type ListIntent =
  | { type: 'setQuery'; query: string }
  | { type: 'select'; ticketId: string | null }
  | { type: 'setPage'; page: number }
  /** Picking the chosen org again turns the filter off. */
  | { type: 'toggleOrg'; name: string }
  | { type: 'clearOrgFilter' }
  | { type: 'refresh' }
  | { type: 'clearNotice' }
  /** The row pill: selects the ticket, then assigns it to you. */
  | { type: 'rowAssignToMe'; ticketId: string }

export interface ListResults {
  setQuery: null
  select: null
  setPage: null
  toggleOrg: null
  clearOrgFilter: null
  refresh: null
  clearNotice: null
  rowAssignToMe: null
}

// ---------------------------------------------------------------------------------------------
// detail:mine, detail:team (the selected ticket of that list)
// ---------------------------------------------------------------------------------------------

export type TimerButton =
  | { type: 'none' }
  /** Stop button with the live counter from `timer.running.counter`. */
  | { type: 'running' }
  /** "Web stopwatch since 10:42", no button. */
  | { type: 'webStopwatch'; label: string }
  | { type: 'start'; enabled: boolean; help: string }

export type PrimaryKind = 'assignToMe' | 'openAssign' | 'openResolve' | 'confirmClose'

export type PrimaryAction =
  | { type: 'none' }
  | {
      type: 'action'
      /** assignToMe → dispatch `assignToMe`; openAssign/openResolve → open that sheet; confirmClose → ask, then `close`. */
      kind: PrimaryKind
      label: string
      help: string
      /** Teal only when the ticket needs you (never for Close). */
      teal: boolean
      enabled: boolean
      /** Mini spinner: a change runs on this ticket. */
      busy: boolean
    }

export interface MoreMenu {
  /** "Assign to someone else…" / "Reassign…" (opens the Assign sheet); null when hidden. */
  assign: { label: string } | null
  /** "Set to pending…" */
  pending: boolean
  /** "Reopen ticket" */
  reopen: boolean
  /** The three items above are enabled. */
  enabled: boolean
  /** A divider after them (the state has any action). */
  divider: boolean
  /** "Open in iTop on the web". */
  webUrl: string
}

export interface DetailStrip {
  /** Caller, or "None". */
  caller: string
  /** Organization, or "None". */
  organization: string
  /** Team, or "No team". */
  team: string
  agent: AgentChip
  /** "1h 20m spent" when the total is > 0. */
  spent: string | null
  /** Only once the detail has loaded. */
  linkedCIs: LinkedCI[]
}

export type DetailBody =
  | { type: 'loading' }
  /** "Couldn't load this ticket. <error>" + Retry. */
  | { type: 'failed'; text: string }
  | {
      type: 'loaded'
      /** "Network access › VPN"; null without a service. */
      serviceLine: string | null
      /** null shows "No description.". */
      description: RichContent | null
      tab: LogTab
      entries: LogEntryView[]
      /** Shown when the selected log is empty. */
      emptyLogText: string | null
    }

export type Attachments =
  | { type: 'loading' }
  /** "Couldn't list attachments. <error>". */
  | { type: 'failed'; text: string }
  | {
      type: 'loaded'
      items: { id: string; name: string }[]
      /** Spinner on this chip; all chips disabled while set. */
      downloadingId: string | null
      /** "Saved vpn-client.log (2 KB)." */
      saved: string | null
      /** "Couldn't download <name>. <error>". */
      error: string | null
    }

export interface Composer {
  isPrivate: boolean
  /** The draft for this ticket and log ("<ticketId>|public"). */
  draft: string
  images: DraftImage[]
  /** "1 image to add" / "3 images to add". */
  imagesAccessibilityLabel: string
  placeholder: string
  accessibilityLabel: string
  sendLabel: string
  sendHelp: string
  editable: boolean
  canSend: boolean
  sending: boolean
  canAddImages: boolean
  paperclipHelp: string
  /** The status line after the paperclip: error, "Uploading 2 images to iTop", or writes-off text. */
  status: Notice | null
  /** Files bigger than this are not read; send them with `dataBase64: null`. */
  maxIntakeFileBytes: number
}

export interface TicketDetailView {
  type: 'ticket'
  /** The list's latest copy of the ticket. */
  ticket: Ticket
  /** "User request" / "Incident". */
  classLabel: string
  /** "✓ Copied <ref>" chip (2 s). */
  copied: boolean
  timerButton: TimerButton
  primary: PrimaryAction
  more: MoreMenu
  closeConfirm: { title: string; message: string }
  strip: DetailStrip
  body: DetailBody
  attachments: Attachments
  /** null until the detail has loaded (the composer appears then). */
  composer: Composer | null
}

export type DetailSnapshot =
  | { type: 'none'; placeholder: string }
  | TicketDetailView

export type DetailIntent =
  | { type: 'retry' }
  | { type: 'setTab'; tab: LogTab }
  | { type: 'copyRef' }
  | { type: 'assignToMe' }
  | { type: 'reopen' }
  /** After the confirmation dialog. */
  | { type: 'close' }
  /** ⌘T: start/stop the timer on this ticket when the timer button is there. */
  | { type: 'toggleTimer' }
  | { type: 'setDraft'; text: string }
  | { type: 'addImages'; items: ImageIntakeItem[] }
  | { type: 'removeImage'; imageId: string }
  | { type: 'send' }
  /** Downloads through REST, then Rust shows the Save panel and writes the file. */
  | { type: 'downloadAttachment'; attachmentId: string }

export interface DetailResults {
  retry: null
  setTab: null
  copyRef: null
  assignToMe: null
  reopen: null
  close: null
  toggleTimer: null
  setDraft: null
  addImages: null
  removeImage: null
  /** true when the entry was added (draft cleared). */
  send: boolean
  downloadAttachment: null
}

// ---------------------------------------------------------------------------------------------
// assign:<scope>, pending:<scope>, resolve:<scope>
// ---------------------------------------------------------------------------------------------

export type SheetClosed = { type: 'closed' }

export interface AssignSheetOpen {
  type: 'open'
  ticketId: string
  /** "Assign R-000436" / "Reassign R-000412". */
  title: string
  subtitle: string
  assignToMe: { enabled: boolean; help: string }
  team: Dropdown
  agent: Dropdown
  /** "Assign ticket" / "Reassign ticket". */
  confirmLabel: string
  confirmEnabled: boolean
  busy: boolean
  error: string | null
}

export type AssignSheetSnapshot = SheetClosed | AssignSheetOpen

export type AssignSheetIntent =
  | { type: 'open'; ticketId: string }
  | { type: 'close' }
  | { type: 'assignToMe' }
  | { type: 'setTeam'; teamId: string | null }
  | { type: 'setAgent'; agentId: string | null }
  | { type: 'confirm' }

export interface AssignSheetResults {
  open: null
  close: null
  /**
   * The Swift method's return: the error text shown in the sheet, or null. Null also when nothing
   * was sent (validation failed: see the snapshot's required marks). The sheet closes itself on
   * success; the UI follows the snapshot, never the result.
   */
  assignToMe: string | null
  setTeam: null
  setAgent: null
  /** As assignToMe. "Another assignment is still running." when one runs. */
  confirm: string | null
}

export interface PendingSheetOpen {
  type: 'open'
  ticketId: string
  title: string
  subtitle: string
  reason: string
  reasonRequired: RequiredMark
  /** "Enter why the ticket is waiting." */
  reasonError: string | null
  /** "Your timer stops and saves 0:42 to this ticket." (re-made every 30 s). */
  timerLine: string | null
  error: string | null
  busy: boolean
  confirmEnabled: boolean
}

export type PendingSheetSnapshot = SheetClosed | PendingSheetOpen

export type PendingSheetIntent =
  | { type: 'open'; ticketId: string }
  | { type: 'close' }
  | { type: 'setReason'; reason: string }
  | { type: 'confirm' }

export interface PendingSheetResults {
  open: null
  close: null
  setReason: null
  /** As AssignSheetResults.confirm. */
  confirm: string | null
}

export interface ResolveSheetOpen {
  type: 'open'
  ticketId: string
  title: string
  subtitle: string
  service: Dropdown
  serviceRequired: RequiredMark
  subcategory: Dropdown
  subcategoryRequired: RequiredMark
  code: Dropdown
  solution: string
  solutionRequired: RequiredMark
  /** "Describe what fixed it. The caller sees this." */
  solutionError: string | null
  timerLine: string | null
  error: string | null
  busy: boolean
  confirmEnabled: boolean
}

export type ResolveSheetSnapshot = SheetClosed | ResolveSheetOpen

export type ResolveSheetIntent =
  | { type: 'open'; ticketId: string }
  | { type: 'close' }
  | { type: 'setService'; serviceId: string | null }
  | { type: 'setSubcategory'; subcategoryId: string | null }
  | { type: 'setCode'; codeId: string | null }
  | { type: 'setSolution'; solution: string }
  | { type: 'confirm' }

export interface ResolveSheetResults {
  open: null
  close: null
  setService: null
  setSubcategory: null
  setCode: null
  setSolution: null
  /** As AssignSheetResults.confirm. */
  confirm: string | null
}

// ---------------------------------------------------------------------------------------------
// newTicket:sheet, newTicket:window
// ---------------------------------------------------------------------------------------------

export interface NewTicketOpen {
  type: 'open'
  /** "On <CI name>" when started from a CI. */
  subtitle: string | null
  ticketClass: TicketClass
  organization: Dropdown
  caller: Dropdown
  service: Dropdown
  subcategory: Dropdown
  impact: Dropdown
  urgency: Dropdown
  title: string
  description: string
  assignToMe: boolean
  required: {
    organization: RequiredMark
    caller: RequiredMark
    service: RequiredMark
    subcategory: RequiredMark
    title: RequiredMark
    description: RequiredMark
  }
  /** "Fill in the caller first." */
  missingText: string | null
  error: string | null
  /** "Creating tickets is switched off in this build, so nothing is sent to iTop." */
  disabledText: string | null
  creating: boolean
  confirmEnabled: boolean
  /** A kept draft exists: show "Clear form". */
  showClearForm: boolean
}

export type NewTicketSnapshot = SheetClosed | NewTicketOpen

export type NewTicketIntent =
  /** Plain start (restores a kept draft). Org/CI starts go through the shell intents. */
  | { type: 'open' }
  | { type: 'setClass'; ticketClass: TicketClass }
  | { type: 'setOrganization'; id: string | null }
  | { type: 'setCaller'; id: string | null }
  | { type: 'setService'; id: string | null }
  | { type: 'setSubcategory'; id: string | null }
  | { type: 'setImpact'; id: string | null }
  | { type: 'setUrgency'; id: string | null }
  | { type: 'setTitle'; title: string }
  | { type: 'setDescription'; description: string }
  | { type: 'setAssignToMe'; assignToMe: boolean }
  | { type: 'clearForm' }
  /** Cancel / Esc: keeps the draft unless org, title and description are empty. */
  | { type: 'cancel' }
  /** The window's close button: no draft kept. */
  | { type: 'dismiss' }
  | { type: 'create' }

export interface NewTicketResults {
  open: null
  setClass: null
  setOrganization: null
  setCaller: null
  setService: null
  setSubcategory: null
  setImpact: null
  setUrgency: null
  setTitle: null
  setDescription: null
  setAssignToMe: null
  clearForm: null
  cancel: null
  dismiss: null
  /**
   * The error text shown under the form, or null: created (the form closes itself), or nothing
   * sent because a field is missing (`missingText` in the snapshot) or creation is switched off.
   */
  create: string | null
}

// ---------------------------------------------------------------------------------------------
// timer
// ---------------------------------------------------------------------------------------------

export interface RunningTimerView {
  ticketId: string
  ref: string
  title: string
  startedAt: Rfc3339
  elapsedSeconds: number
  /** "00:12:34". */
  counter: string
  /** "0:12". */
  clock: string
}

export interface TimerNotice {
  text: string
  isError: boolean
  canRetry: boolean
  /** "Discard this time" when retryable, else "Dismiss" (tooltip and a11y of the ×). */
  dismissLabel: string
}

export interface TimerSnapshot {
  /** Re-emitted every second while set. */
  running: RunningTimerView | null
  saving: boolean
  notice: TimerNotice | null
}

export type TimerIntent =
  | { type: 'start'; ticketId: string }
  | { type: 'stop' }
  | { type: 'toggle'; ticketId: string }
  | { type: 'retry' }
  | { type: 'clearNotice' }

export interface TimerResults {
  start: null
  stop: null
  toggle: null
  retry: null
  clearNotice: null
}

// ---------------------------------------------------------------------------------------------
// cmdb
// ---------------------------------------------------------------------------------------------

export interface CIRow {
  /** "<Class>::<key>". */
  id: string
  key: string
  className: string
  name: string
  /** "<Class label> · <Org>". */
  subtitle: string
}

export interface CIChip {
  key: string
  className: string
  name: string
  /** "Network device: open vpn-gw-01". */
  help: string
}

export type CIResults =
  | { type: 'idle'; text: string }
  | { type: 'searching' }
  | { type: 'failed'; text: string }
  | { type: 'empty'; text: string }
  /** `header`: "1 result", "3 results", "First 50 results". */
  | { type: 'loaded'; header: string; items: CIRow[] }

export type CIDetailBody =
  | { type: 'loading'; text: string }
  | { type: 'failed'; text: string }
  | {
      type: 'loaded'
      fields: { label: string; value: string }[]
      description: string | null
      dependsOn: CIChip[]
      usedBy: CIChip[]
      /** "Nothing in the CMDB depends on X." / "If X fails, A and B are affected." */
      usedBySentence: string
      openTickets: CITicketRow[]
      /** "No open tickets on X." */
      openTicketsEmptyText: string | null
    }

export interface CIDetailView {
  id: string
  key: string
  className: string
  name: string
  orgName: string
  /** "<Class label> · <Status>" (status once loaded). */
  headerLine: string
  copied: boolean
  webUrl: string
  body: CIDetailBody
}

export interface CMDBSnapshot {
  query: string
  /** "Config items in <org>" bar. */
  scope: { orgId: string; orgName: string } | null
  results: CIResults
  selectedId: string | null
  current: CIDetailView | null
  placeholder: string
}

export type CMDBIntent =
  | { type: 'setQuery'; query: string }
  | { type: 'clearScope' }
  | { type: 'select'; id: string | null }
  /** A relationship chip. */
  | { type: 'openCI'; key: string; className: string; name: string }
  | { type: 'retryDetail' }
  | { type: 'copyName' }

export interface CMDBResults {
  setQuery: null
  clearScope: null
  select: null
  openCI: null
  retryDetail: null
  copyName: null
}

// ---------------------------------------------------------------------------------------------
// orgs
// ---------------------------------------------------------------------------------------------

export interface OrgRow {
  id: string
  name: string
  code: string
  openCount: number
}

export type OrgList =
  | { type: 'loading' }
  | { type: 'failed'; text: string }
  | { type: 'empty'; text: string }
  | { type: 'loaded'; orgs: OrgRow[] }

export interface OrgDetailView {
  id: string
  name: string
  code: string
  /** "Organization · A" or "Organization". */
  headerLine: string
  /** "People 6" or "People". */
  peopleLabel: string
  /** "Config items 3" or "Config items". */
  configItemsLabel: string
  /** "Open tickets" / "Tickets". */
  ticketsHeading: string
  tickets: TableState
}

export interface OrgsSnapshot {
  query: string
  list: OrgList
  selectedId: string | null
  detail: OrgDetailView | null
  placeholder: string
}

export type OrgsIntent =
  | { type: 'setQuery'; query: string }
  | { type: 'select'; id: string | null }
  | { type: 'retryList' }
  | { type: 'retryTickets' }
  | { type: 'setTicketPage'; page: number }

export interface OrgsResults {
  setQuery: null
  select: null
  retryList: null
  retryTickets: null
  setTicketPage: null
}

// ---------------------------------------------------------------------------------------------
// people
// ---------------------------------------------------------------------------------------------

export interface PersonRow {
  id: string
  displayName: string
  initials: string
  /** "Agent · Network, Service desk" / "Caller · Org A". */
  subtitle: string
}

export type PeopleResults =
  | { type: 'idle'; text: string }
  | { type: 'searching' }
  | { type: 'failed'; text: string }
  | { type: 'empty'; text: string }
  /** `banner`: "Showing 100 of 240 people. …" when more were found than shown. */
  | { type: 'loaded'; banner: string | null; people: PersonRow[] }

export interface PersonDetailView {
  id: string
  displayName: string
  initials: string
  /** "Agent · IT support", "Caller". */
  kindLine: string
  organization: { orgId: string; orgName: string } | null
  email: string | null
  phone: string | null
  /** Agents only: "Network, Service desk". */
  teams: string | null
  tab: PeopleTab
  /** "Assigned to them 3" (count once loaded). */
  assignedLabel: string
  reportedLabel: string
  tickets: TableState
}

export interface PeopleSnapshot {
  query: string
  scope: { orgId: string; orgName: string } | null
  results: PeopleResults
  selectedId: string | null
  detail: PersonDetailView | null
  placeholder: string
}

export type PeopleIntent =
  | { type: 'setQuery'; query: string }
  | { type: 'clearScope' }
  | { type: 'select'; id: string | null }
  | { type: 'setTab'; tab: PeopleTab }
  | { type: 'setTicketPage'; page: number }

export interface PeopleResultsMap {
  setQuery: null
  clearScope: null
  select: null
  setTab: null
  setTicketPage: null
}

// ---------------------------------------------------------------------------------------------
// menubar (tray title and the panel)
// ---------------------------------------------------------------------------------------------

export type MenuRowRight = { type: 'none' } | { type: 'running' } | { type: 'spent'; text: string }

export type MenuRowButton =
  | { type: 'stop' }
  | { type: 'start'; enabled: boolean; help: string }
  /** Empty 22 pt slot so times line up. */
  | { type: 'slot' }

export interface MenuRow {
  ticketId: string
  ref: string
  title: string
  needsYou: boolean
  /** Tinted row with the live counter from `timer.running.counter`. */
  isRunning: boolean
  right: MenuRowRight
  button: MenuRowButton
}

export type MenuContent =
  | { type: 'loading' }
  | { type: 'failed'; text: string }
  | { type: 'empty'; text: string }
  | {
      type: 'list'
      /** The running ticket under "Running", when it's listed. */
      running: MenuRow | null
      /** Titles like "Assigned  3" (two spaces). */
      sections: { group: TicketGroup; title: string; rows: MenuRow[] }[]
      /** Scroll height in points: min(420, …). */
      height: number
    }

export interface MenuBarSnapshot {
  /** The tray title after the icon: "00:12:40  3", or "". Rust draws it; here for reference. */
  trayTitle: string
  /** "iTop Wrap: 3 tickets need you, timer 0:12 on R-000412". */
  trayAccessibilityLabel: string
  /** The panel's own search. */
  query: string
  content: MenuContent
  refreshing: boolean
  /** "Updated 2 minutes ago"; null before the first load. */
  updatedText: string | null
}

export type MenuBarIntent =
  | { type: 'setQuery'; query: string }
  /** Main window forward, My tickets, its search cleared, the ticket selected. */
  | { type: 'openTicket'; ticketId: string }
  /** ⌘O: main window forward, selection unchanged. */
  | { type: 'openMain' }
  /** ⌘N: the separate New Ticket window. */
  | { type: 'newTicket' }
  /** ⌘R: reload My tickets. */
  | { type: 'refresh' }

export interface MenuBarResults {
  setQuery: null
  openTicket: null
  openMain: null
  newTicket: null
  refresh: null
}

// ---------------------------------------------------------------------------------------------
// The model map
// ---------------------------------------------------------------------------------------------

/** Checks at compile time that every intent has exactly one result entry. */
type Model<S, I extends { type: string }, R extends { [K in I['type']]: unknown }> = {
  snapshot: S
  intent: I
  results: R
}

export interface ModelMap {
  session: Model<SessionSnapshot, SessionIntent, SessionResults>
  shell: Model<ShellSnapshot, ShellIntent, ShellResults>
  options: Model<OptionsSnapshot, OptionsIntent, OptionsResults>
  'list:mine': Model<ListSnapshot, ListIntent, ListResults>
  'list:team': Model<ListSnapshot, ListIntent, ListResults>
  'detail:mine': Model<DetailSnapshot, DetailIntent, DetailResults>
  'detail:team': Model<DetailSnapshot, DetailIntent, DetailResults>
  'assign:mine': Model<AssignSheetSnapshot, AssignSheetIntent, AssignSheetResults>
  'assign:team': Model<AssignSheetSnapshot, AssignSheetIntent, AssignSheetResults>
  'pending:mine': Model<PendingSheetSnapshot, PendingSheetIntent, PendingSheetResults>
  'pending:team': Model<PendingSheetSnapshot, PendingSheetIntent, PendingSheetResults>
  'resolve:mine': Model<ResolveSheetSnapshot, ResolveSheetIntent, ResolveSheetResults>
  'resolve:team': Model<ResolveSheetSnapshot, ResolveSheetIntent, ResolveSheetResults>
  'newTicket:sheet': Model<NewTicketSnapshot, NewTicketIntent, NewTicketResults>
  'newTicket:window': Model<NewTicketSnapshot, NewTicketIntent, NewTicketResults>
  timer: Model<TimerSnapshot, TimerIntent, TimerResults>
  cmdb: Model<CMDBSnapshot, CMDBIntent, CMDBResults>
  orgs: Model<OrgsSnapshot, OrgsIntent, OrgsResults>
  people: Model<PeopleSnapshot, PeopleIntent, PeopleResultsMap>
  menubar: Model<MenuBarSnapshot, MenuBarIntent, MenuBarResults>
}

export type ModelId = keyof ModelMap
export type SnapshotOf<K extends ModelId> = ModelMap[K]['snapshot']
export type IntentOf<K extends ModelId> = ModelMap[K]['intent']
export type ResultOf<K extends ModelId, T extends IntentOf<K>['type']> = T extends keyof ModelMap[K]['results']
  ? ModelMap[K]['results'][T]
  : never

/** Every model id, for code that walks all of them. */
export const MODEL_IDS: readonly ModelId[] = [
  'session',
  'shell',
  'options',
  'list:mine',
  'list:team',
  'detail:mine',
  'detail:team',
  'assign:mine',
  'assign:team',
  'pending:mine',
  'pending:team',
  'resolve:mine',
  'resolve:team',
  'newTicket:sheet',
  'newTicket:window',
  'timer',
  'cmdb',
  'orgs',
  'people',
  'menubar',
] as const

/** Models that exist only while signed in: their snapshot is null otherwise. */
export const WORKSPACE_MODEL_IDS: readonly ModelId[] = MODEL_IDS.filter((id) => id !== 'session')

// ---------------------------------------------------------------------------------------------
// Transport payloads and errors
// ---------------------------------------------------------------------------------------------

/** Payload of the `snapshot` event. `snapshot` is null when the model stops existing (sign-out). */
export interface SnapshotEvent<K extends ModelId = ModelId> {
  model: K
  snapshot: SnapshotOf<K> | null
}

/**
 * What `get_snapshot` / `dispatch` reject with. Business failures are not errors: they come back as
 * the intent's result (an error string) or in the snapshot, exactly like the Swift methods.
 */
export type AppError =
  /** A workspace model was addressed while signed out. */
  | { type: 'notSignedIn'; message: string }
  | { type: 'unknownModel'; message: string }
  /** The intent didn't deserialize, or names a ticket/item the model doesn't have. */
  | { type: 'invalidIntent'; message: string }
  /** A bug in Rust; the UI logs it. */
  | { type: 'internal'; message: string }

export function isAppError(value: unknown): value is AppError {
  if (typeof value !== 'object' || value === null) return false
  const v = value as { type?: unknown; message?: unknown }
  return (
    typeof v.message === 'string' &&
    (v.type === 'notSignedIn' || v.type === 'unknownModel' || v.type === 'invalidIntent' || v.type === 'internal')
  )
}

/** Native commands that aren't model intents. */
export interface NativeApi {
  /** Opens an http, https or mailto URL in the default app (Rust refuses other schemes). */
  openUrl(url: string): Promise<void>
}
