# Contract: Rust backend ↔ React UI

Status: draft for the rewrite, 2026-11-16. The TypeScript shape is `ui/src/backend/contract.ts`; this
document says where every piece comes from in the Swift app, how long each model lives, what the
native side does, and what is still open. Behaviour is defined by `docs/rewrite/parity-checklist.md`
(item ids like LIST-30 below refer to it). Paths: `W/` = `Sources/ITopWrap/`, `K/` = `Sources/ITopKit/`.

## 1. Rules

1. **Rust owns every model.** Each Swift view model becomes a Rust model with a *snapshot* (everything
   the views read) and an *intent* union (everything the views do to it).
2. **No rule in the UI.** If a Swift view has an `if` that depends on data, its result is in the
   snapshot: visible sections, page and label, need count, org choices, option summary, which actions
   a ticket has and which is primary, timer strings, start/stop availability, web URLs, formatted
   dates and durations, error and notice texts, placeholders of dropdowns.
   *Allowed in React:* fixed copy ("Description", "Retry", "Search config items"), interpolating a
   snapshot field into fixed copy with no branching ("Copy R-000412", "Open R-000412",
   "Linked CI: open vpn-gw-01 in Config items"), and choosing an icon for a tag the snapshot gives.
3. **Purely visual state stays in React** and is not in the contract: which popover, menu, confirmation
   dialog or image viewer is open, hover, focus rings, scroll position, the header's stacked layout
   (DET-11), the drop-target highlight (LOG-15), dropdown open/highlight/filter text (KEY-11…17).
   Exception: the four sheets (Assign, Pending, Resolve, New ticket) are models with an `open`/`closed`
   snapshot, because their data loads and validates in Rust and because the native ⌘N menu opens New
   ticket. React shows a sheet exactly when its model is `open`.
4. **Text fields that the Swift model owns are model state.** `query` of every list/search, the login
   server and user, the composer draft, sheet reasons/solutions, New ticket title/description: React
   sends each change as an intent (`setQuery`, `setDraft`, …) and renders the snapshot's value. Debounce
   (250 ms ref lookup, 300 ms CMDB/People search) is in Rust, like Swift. React may keep a local copy
   while typing for responsiveness but must not apply any rule to it.
5. **Live clocks tick in Rust.** The `timer` snapshot is re-emitted every second while a timer runs;
   rows and buttons that show a live value carry only `{type:"running"}` and the UI reads
   `timer.running.counter` / `.clock`. Lists are re-emitted every 30 s (`updatedText`, ages), sheets'
   `timerLine` every 30 s, the tray title every 1 s with a timer, else at most every 15 s (POLL-8).
6. **Serde-compatible shapes.** camelCase fields; tagged unions `#[serde(tag = "type",
   rename_all = "camelCase")]` with struct or unit variants only; optional = `Option<T>` = `T | null`
   (never a missing field); dates RFC 3339; unit results serialize as `null`.
7. **Business failures are not transport errors.** A refused change, a failed load or a validation
   message comes back exactly as Swift returns it: as the intent's result (`string | null`, `boolean`)
   and/or in the snapshot. Only transport problems reject (section 7).

## 2. Transport

| Direction | Name | Payload | Returns |
|---|---|---|---|
| command | `get_snapshot` | `{ model: ModelId }` | the model's snapshot, or `null` when it doesn't exist |
| command | `dispatch` | `{ model: ModelId, intent: IntentOf<model> }` | `ResultOf<model, intent.type>` |
| command | `open_url` | `{ url: string }` | `null`; refuses anything but `http`, `https`, `mailto` |
| event | `snapshot` | `{ model: ModelId, snapshot: Snapshot \| null }` | — |
| protocol | `itop://localhost/inline-image/<id>` | GET | image bytes with their mimetype; 404 when it can't be loaded |
| protocol | `itop://localhost/draft-image/<uuid>` | GET | a composer image's bytes (thumbnails) |

- Rust emits a **full** new snapshot to every window (`emit`, not `emit_to`) whenever a model changes;
  coalescing within one tick is fine. When a model stops existing (sign-out) it emits `snapshot: null`.
- `dispatch` resolves after the Swift method's equivalent has finished (e.g. `list.refresh` after the
  reload, `detail.send` after the send and the reload), so tests and `await` work like Swift's `async`.
  Snapshots for intermediate states (spinners) are emitted while it runs.
- **Binary data (decided):** a custom URI scheme protocol `itop` (`register_asynchronous_uri_scheme_protocol`)
  serves inline log images (fetched through REST by id, `SELECT InlineImage WHERE id = <n>`, last 60
  cached: DET-44, DET-48) and composer thumbnails. Snapshots carry ready-to-use `src` strings: an
  `itop://localhost/...` URL, or the original `data:image/...` URL for images embedded in the HTML
  (DET-45). The UI never builds asset URLs. Attachments never cross the bridge: `detail.downloadAttachment`
  makes Rust download, show the Save panel and write the file (DET-33).
- **Images into Rust:** paste, drop and the paperclip picker produce `ImageIntakeItem { filename,
  dataBase64, size }` sent with `detail.addImages`. Files larger than `composer.maxIntakeFileBytes`
  (40 MB, LOG-16) are sent with `dataBase64: null` and not read. Raw clipboard image data is sent with
  `filename: ""`; Rust names it "Pasted image.png"/".tiff" (LOG-14). Type sniffing, conversion
  (TIFF→PNG, other→JPEG 0.85, ≤ 2560 px), the 4 MB and 5-image limits and all intake messages are in
  Rust (LOG-17…19).
- The UI's transport (`ui/src/backend/index.ts`) uses Tauri when `window.__TAURI_INTERNALS__` exists,
  else the mock (section 9). `useModel(id)` returns `[snapshot | null, send]`; `null` until the first
  snapshot arrives and while a workspace model doesn't exist.

## 3. Models, ids and lifetimes

| Id | Swift source | Lives | Snapshot fields | Intents |
|---|---|---|---|---|
| `session` | `SessionModel` (W/SessionModel.swift:35), `LoginView`, `UnreachableView`, `RootView` (W/App.swift:140) | always | 4 variants; form 6 | 6 |
| `shell` | `SessionModel.selection` + `ShellView` (W/ShellView.swift) | signed in | 6 | 9 |
| `options` | `ListOptions` (W/ListOptions.swift:8) | signed in | 12 | 5 |
| `list:mine`, `list:team` | `TicketListModel` (W/TicketListModel.swift:52) + `TicketListView` | signed in | 12 (+ 9 in `ListBody`) | 8 |
| `detail:mine`, `detail:team` | `TicketDetailModel` (W/TicketDetailModel.swift:7) + `TicketDetailView`, `LogComposer`, `AttachmentsView`, `TimerButton` | signed in; `{type:"none"}` with no selection | 11 (+ composer 17) | 12 |
| `assign:mine`, `assign:team` | `AssignSheet` @State (W/AssignSheet.swift) | signed in; `closed` until `open` | 10 | 6 |
| `pending:mine`, `pending:team` | `PendingSheet` (W/LifecycleSheets.swift:6) | signed in; `closed`/`open` | 10 | 4 |
| `resolve:mine`, `resolve:team` | `ResolveSheet` (W/LifecycleSheets.swift:44) | signed in; `closed`/`open` | 15 | 7 |
| `newTicket:sheet`, `newTicket:window` | `NewTicketSheet` (W/NewTicketView.swift:37), `Workspace.newTicketStart/Draft` | signed in; `closed`/`open` | 18 | 15 |
| `timer` | `TimerModel` (W/TimerModel.swift:23) | signed in | 3 (`running` 7) | 5 |
| `cmdb` | `CMDBModel` (W/CMDBModel.swift:19) + `CMDBListView`, `CIDetailView` | signed in | 6 | 6 |
| `orgs` | `OrgsModel` (W/OrgsModel.swift:39) + `OrgsListView`, `OrgDetailView`, `TicketTable` | signed in | 5 | 5 |
| `people` | `PeopleModel` (W/OrgsModel.swift:95) + `PeopleListView`, `PersonDetailView` | signed in | 6 | 5 |
| `menubar` | `MenuBarPanel`, `MenuBarLabel`, `MenuBarStatus` (W/MenuBarView.swift, K/MenuBarStatus.swift) | signed in | 6 | 5 |

20 ids, 14 model types.

**Why detail is per list.** Swift creates `TicketDetailModel` only in `ShellView` for the selected
ticket of the current list (`TicketDetailView(ticket:list:)`, W/ShellView.swift:48-50, recreated by
`.id(t.id)`). Opening a ticket from a CI, an org, a person or the menu bar never creates a detail of its
own: it routes into My/Team tickets (`openTicket`, W/ShellView.swift:120-133; W/App.swift:102-110), and
the list's detail shows it. So there are exactly two detail models, keyed by list. A detail model's
state (log tab, "Copied" chip, send error, attachments) is reset whenever its list's selected ticket id
changes; drafts and draft images live in the list (LOG-4) and survive that.

**Workspace lifetime.** All models but `session` exist while signed in (`Workspace`, W/Workspace.swift).
While signed out their snapshot is `null` and their intents reject with `notSignedIn`. Rust keeps the
workspace for a sign-in by the same user on the same server (AUTH-32) and re-emits it then.

**Sheets per list.** Swift sheets read `list.working`, `list.timer` and call list methods, so each list
has its own Assign/Pending/Resolve model. Only one sheet shows at a time; React opens a sheet by
dispatching `open { ticketId }` and closes it with `close`; Rust closes it itself on success. The
ticket a sheet shows is the list's latest copy by id (as Swift re-reads `ticket` on each render).

**Two New ticket instances.** The main-window sheet and the menu bar's separate window are independent
forms (separate @State in Swift) sharing one kept draft (`Workspace.newTicketDraft`) and the
"Assign to me" preference.

## 4. Shared records (contract.ts "Shared records")

| Record | Fields | Mirrors |
|---|---|---|
| `Ticket` | id, key, ticketClass, ref, title, status, statusLabel, priority, callerName, orgName, orgId, teamId, teamName, agentId, agentName, startDate, lastUpdate, isUnassigned, isMine, needsYou | `Ticket` (K/Ticket.swift:13-51); `statusLabel` = `TicketGroup.statusLabel` (W/TicketListView.swift:386); `needsYou` = `TicketList.needsYou` (K/TicketQueries.swift:159); `isMine` = `agentID == me.contactID` |
| `TicketRow` | ticket, age, openedTooltip, priorityMark, agent (`RowAgent`), time (`RowTime`) | `TicketRow` (W/TicketListView.swift:284-381): `Age.short` (K/TicketQueries.swift:250), "Opened …" tooltip (:308), priority mark (:343), agent rules incl. `assignAction` (W/TicketListView.swift:91, 182-188, 357-381), time (:317-334) |
| `TicketSection` | group, title, total, rows | `TicketSection` + `TicketPage.groupTotal` (K/TicketQueries.swift:151, 220) |
| `Pager` | none / count {text} / pages {page, pageCount, label, canPrevious, canNext} | `Pager` view (W/ListOptions.swift:156-178), `TicketPage` (K/TicketQueries.swift:183-224) |
| `RichContent` / `RichBlock` / `RichRun` | blocks: text {runs} / image {src, ticketWebUrl}; runs: text {text, link} / icon / outsideImage {host, url} | `RichText.parse` (K/RichText.swift:50), `RichTextView` (W/RichTextView.swift:15-66): empty parse → Rust sends one text block with the plain text (DET-50) |
| `LogEntryView` | id, header, isPrivate, content | `LogEntry` (K/CaseLog.swift:4), header line (W/TicketDetailView.swift:321), `LogDate.short` (K/CaseLog.swift:98) |
| `LinkedCI` | key, className, name | `LinkedCI` (K/TicketDetail.swift:3) |
| `Dropdown` / `PickItem` | items, selectedId, placeholder, enabled, search {prompt, noMatch} | `DropdownField` call sites; placeholders = each sheet's `placeholder(_:)` (W/AssignSheet.swift:100, W/LifecycleSheets.swift:106, W/NewTicketView.swift:165) |
| `RequiredMark` | missing | `RequiredLabel(missing: tried && empty)` (W/LifecycleSheets.swift:181) |
| `TicketTable` / `TableSection` / `TableRow` | lastColumn, sections, pager; row: ticketId, ref, title, needsYou, p1, callerName, assignee | `TicketTable` (W/OrgsView.swift:7-86), `PersonChip` (:89) |
| `AgentChip` | unassigned / person {initials, name} | detail strip agent (W/TicketDetailView.swift:255-282), `PersonChip` |
| `CITicketRow` | ticketId, ref, title, needsYou, statusLine | CI open tickets row (W/CMDBView.swift:214-231) |
| `Notice` | text, isError | `TicketListModel.Notice` (W/TicketListModel.swift:211) |
| `ImageIntakeItem` / `DraftImage` | filename, dataBase64, size / id, filename, sizeText, thumbnailSrc | `ImageIntake.Item` (W/ComposerTextView.swift), `DraftImageThumbnail` (W/TicketDetailView.swift:514-565) |

## 5. Models

Each table lists the snapshot fields (type as in contract.ts), what they mirror, and the intents with
payload, result and the Swift method.

### 5.1 `session`

Snapshot `SessionSnapshot` (W/App.swift:140-157 `RootView`, W/SessionModel.swift:37-42 `State`).

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `type` | `restoring` \| `signedOut` \| `unreachable` \| `signedIn` | `SessionModel.state` | W/SessionModel.swift:37-44 |
| `form.server` | string | `SessionModel.server` (saved or "https://") | W/SessionModel.swift:50, 57 |
| `form.user` | string | `SessionModel.user` | W/SessionModel.swift:51, 58 |
| `form.httpWarning` | boolean | `isPlainHTTP` (AUTH-13) | W/LoginView.swift:77-79 |
| `form.signingIn` | boolean | `signing` (button spinner, "Signing in…") | W/LoginView.swift:56-63 |
| `form.message` | `LoginMessage \| null` | `message` priority: missing fields, REST box, error, notice (AUTH-15…19) | W/LoginView.swift:82-99 |
| `form.initialFocus` | `server`\|`user`\|`password` | `onAppear` focus rule (AUTH-12) | W/LoginView.swift:72-74 |
| `message` (unreachable) | string | `.unreachable(error.localizedDescription)` | W/SessionModel.swift:76-77 |
| `displayName`, `host` (signedIn) | string | `session.user.displayName`, server host | W/ShellView.swift:143 |

| Intent | Payload | Result | Swift |
|---|---|---|---|
| `setServer` | server | null | `model.server` binding (W/LoginView.swift) |
| `setUser` | user | null | `model.user` binding |
| `signIn` | password | `SignInResult {ok, focus}` | `LoginView.submit` + `SessionModel.signIn(password:)` (W/LoginView.swift:108-129, W/SessionModel.swift:82-87). The password crosses the bridge once and is only stored in the Keychain by Rust; it never appears in a snapshot. |
| `retryRestore` | — | null | "Try again" → `restore()` (W/ShellView.swift:199) |
| `signInWithAnotherAccount` | — | null | `forgetAndSignIn()` (W/SessionModel.swift:114) |
| `signOut` | — | null | `signOut()` after the confirmation dialog (W/SessionModel.swift:108; W/ShellView.swift:95-99) |

Launch: Rust starts in `restoring` and runs restore (AUTH-1…4).

### 5.2 `shell`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `selection` | `SidebarItem` | `SessionModel.selection` | W/SessionModel.swift:49 |
| `sidebar` | `SidebarRow[] {item, title, count}` | `SidebarItem.title` + `row(_:count:)`; count only My tickets, > 0 (SHELL-3, SHELL-4) | W/SessionModel.swift:5-28, W/ShellView.swift:21-31, 145-160 |
| `contentTitle` | string | `.navigationTitle(model.selection.title)` | W/ShellView.swift:44 |
| `needCount` | number | `mine.needCount` | W/TicketListModel.swift:108 |
| `account` | `{initials, displayName, host}` | account button and menu (AUTH-27, AUTH-28); host fallback "iTop" | W/ShellView.swift:143, 162-186 |
| `signOutConfirm` | `{title, message}` | confirmation dialog texts (AUTH-29) | W/ShellView.swift:95-99 |

| Intent | Payload | Result | Swift |
|---|---|---|---|
| `select` | item | null | sidebar selection / View ▸ ⌘1…5; also runs the section's on-appear loads (POLL-3: list loads if not loaded, Team starts polling, org list loads once) |
| `openTicket` | `TicketLink {ticketId, ref}` | null | `ShellView.openTicket` (W/ShellView.swift:120-133, SHELL-9) |
| `openOrg` | orgId, orgName | null | `openOrg` (W/ShellView.swift:107-111, SHELL-10) |
| `openCI` | `LinkedCI` | null | `openCI` (W/ShellView.swift:114-117, SHELL-11) |
| `openOrgPeople` | orgId, orgName | null | `people.showOrg` + select People (W/ShellView.swift:68) |
| `openOrgConfigItems` | orgId, orgName | null | `cmdb.showOrg` + select Config items (W/ShellView.swift:69) |
| `newTicket` | — | null | `workspace.requestNewTicket()` (W/Workspace.swift:56) |
| `newTicketForOrg` | orgId, orgName | null | `NewTicketStart(orgID:orgName:)` (W/ShellView.swift:70) |
| `newTicketForCI` | key, name, orgName | null | `NewTicketStart(orgName:ciID:ciName:)` (W/ShellView.swift:59) |

The detail placeholders (SHELL-7) are in each section's model (`detail.placeholder`, `cmdb.placeholder`, …).

### 5.3 `options`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `range` | `0\|7\|30\|90` | `ListOptions.range` | W/ListOptions.swift:25 |
| `includeClosed` | boolean | `includeClosed` | :26 |
| `sort` | `TicketSort` | `sort` | :27 |
| `pageSize` | number \| null | `pageSize` (nil = All) | :29 |
| `isDefault` | boolean | `isDefault` | :47 |
| `summary` | string \| null | `summary` (LIST-42) | :57-73 |
| `help` | string | View button `.help` (LIST-43) | :116 |
| `accessibilityLabel` | string | View button a11y | :117 |
| `rangeChoices` | `Choice<RangeDays>[]` | `Range.allCases` titles | :9-21, 84-86 |
| `sortChoices` | `Choice<TicketSort>[]` | `TicketSort.title` | K/TicketQueries.swift:31-40 |
| `pageSizeChoices` | `Choice<number\|null>[]` | `pageSizes` "10 per page" … "All" | W/ListOptions.swift:30, 99-101 |

| Intent | Payload | Result | Swift |
|---|---|---|---|
| `setRange` | range | null | `options.range` (+ the shown view reloads and goes to page 1: LIST-46) |
| `setIncludeClosed` | includeClosed | null | `options.includeClosed` (same) |
| `setSort` | sort | null | `options.sort` (shown list page 1) |
| `setPageSize` | pageSize | null | `options.pageSize` (shown list page 1) |
| `reset` | — | null | `reset()` "Back to default view" (W/ListOptions.swift:49) |

Swift reacts to option changes in the *views* (`onChange` in `TicketListView`, `OrgDetailView`,
`PersonDetailView`, `TicketTable`), so only the section on screen reloads or resets its page. Rust
mirrors that using `shell.selection` (see Open points). Every change is persisted at once (PERS-9).

### 5.4 `list:mine`, `list:team`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `scope` | `mine`\|`team` | `view` | W/TicketListModel.swift:67 |
| `query` | string | `query` | :85 |
| `selection` | string \| null | `selection` | :86 |
| `content` | `ListContent` | `content` switch: loading / failed / no teams / empty (LIST-44, LIST-60…63) / list | W/TicketListView.swift:47-68 |
| `content.body.orgNoMatch` | `{text, seeAll}` \| null | `orgNoMatch` + `orgID(named:)` (LIST-37) | W/TicketListView.swift:72-74, 158-169; W/TicketListModel.swift:119 |
| `content.body.refreshError` | string \| null | "Couldn't refresh: …" (LIST-62) | W/TicketListView.swift:75-79 |
| `content.body.lookupBar` | `LookupBar` \| null | `lookupStatus` searching/missing/failed (LIST-24…26) | W/TicketListView.swift:193-200 |
| `content.body.showNotices` | boolean | notices only when lookup is `.none`/`.found` (LIST-69) | W/TicketListView.swift:201 |
| `content.body.notice` | `Notice` \| null | `notice` | W/TicketListModel.swift:209 |
| `content.body.extraSection` | `{title, row}` \| null | `lookupSection`: "Search result: not in your queue" or "<Status> just now" (LIST-27, LIST-30) | W/TicketListView.swift:173-179 |
| `content.body.noMatchText` | string \| null | "No tickets in your list match “…”." (LIST-18) | W/TicketListView.swift:83-87 |
| `content.body.sections` | `TicketSection[]` | `currentPage.sections` after org filter → search → group → page (LIST-17) | W/TicketListModel.swift:110-114 |
| `showControls` | boolean | `phase == .loaded && !hasNoTeams` (LIST-45) | W/TicketListView.swift:15 |
| `orgFilter` | `OrgFilter` | menu visibility (`openOrg != nil`, `orgChoices.count > 1 \|\| orgFilter != nil`), label, help, a11y, choices (LIST-33…36) | W/TicketListView.swift:116, 133-155; W/TicketListModel.swift:116 |
| `pager` | `Pager` | footer `Pager` when loaded (LIST-49) | W/TicketListView.swift:262 |
| `refreshing` | boolean | `refreshing` (Refresh disabled) | W/TicketListModel.swift:76; W/TicketListView.swift:32 |
| `footerSpinner` | boolean | `refreshing && phase == .loaded` | W/TicketListView.swift:264 |
| `updatedText` | string | `updatedText(now:)` (LIST-68) | W/TicketListView.swift:276-280 |
| `needCount` | number | `needCount` | W/TicketListModel.swift:108 |
| `working` | string \| null | `working` | :207 |

| Intent | Payload | Result | Swift |
|---|---|---|---|
| `setQuery` | query | null | `query` binding + `onChange` page 1 + `.task(id: query) searchChanged()` (W/TicketListView.swift:18-21; W/TicketListModel.swift:182) |
| `select` | ticketId \| null | null | `selection` binding (SHELL-8: no deferral needed) |
| `setPage` | page | null | `page` binding via `Pager` |
| `toggleOrg` | name | null | `toggleOrg(_:)` (W/TicketListModel.swift:123) + page 1 |
| `clearOrgFilter` | — | null | "All organizations" (`orgFilter = nil`, W/TicketListView.swift:135) |
| `refresh` | — | null | `refresh()` — toolbar ⌘R, Retry (W/TicketListModel.swift:139) |
| `clearNotice` | — | null | `clearNotice()` (:216) |
| `rowAssignToMe` | ticketId | null | row pill: `selection = id; assignToMe(t)` (W/TicketListView.swift:182-188) |

Polling (Rust): My tickets at session start and every 120 s; Team tickets every 120 s only while
`shell.selection == teamTickets` (POLL-1, POLL-2). After each reload: lookup refresh (LIST-29), stale
selection cleared (LIST-66), timer check on My tickets (TMR-14), totals refresh (TMR-17).

### 5.5 `detail:mine`, `detail:team`

`{type:"none", placeholder}` ("Select a ticket to see its details.", SHELL-7) or a `TicketDetailView`:

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `ticket` | `Ticket` | `list.ticket(model.ticket.id) ?? model.ticket` (DET-1) | W/TicketDetailView.swift:25 |
| `classLabel` | string | "User request" / "Incident" (DET-7) | :92-94 |
| `copied` | boolean | `model.copied` (2 s, DET-10) | W/TicketDetailModel.swift:18, 98-103 |
| `timerButton` | `TimerButton` | `TimerButton` body: running / web stopwatch / start (enabled, help) / none (TMR-19) | W/TimerButton.swift:11-42 |
| `primary` | `PrimaryAction` | `primaryAction` + `actionButton`: kind, label, help, teal, enabled, busy (DET-13…18) | W/TicketDetailView.swift:185-218 |
| `more` | `MoreMenu` | More menu items, enabled, divider, web URL (DET-19…24) | :122-159; W/TicketListModel.swift:348-354 |
| `closeConfirm` | `{title, message}` | Close confirmation dialog (DET-16) | W/TicketDetailView.swift:175-180 |
| `strip` | `DetailStrip` | caller/org "None", team "No team", agent chip, "<total> spent", linked CIs after load (DET-25…28) | :222-282 |
| `body` | `DetailBody` | phase loading / failed "Couldn't load this ticket. …" / loaded: service line, description or "No description.", tab, entries, empty-log text (DET-4…6, DET-30, DET-31, DET-35…39) | :32-53, 286-333 |
| `attachments` | `Attachments` | `AttachmentsView` items, downloading, saved, errors (DET-32…34) | W/RichTextView.swift:129-194 |
| `composer` | `Composer` \| null | `LogComposer`, shown once loaded (LOG-1…13, LOG-20) | W/TicketDetailView.swift:60-62, 383-511 |
| `composer.isPrivate` | boolean | `model.tab == .private` | :390 |
| `composer.draft` | string | `list.drafts["<id>\|<log>"]` | :391-394 |
| `composer.images` | `DraftImage[]` | `list.draftImages[key]` | :395 |
| `composer.imagesAccessibilityLabel` | string | "1 image to add" / "<n> images to add" | :440 |
| `composer.placeholder`, `.accessibilityLabel`, `.sendLabel` | string | public/private texts (LOG-2, LOG-5) | :417-428, 483 |
| `composer.sendHelp` | string | "Add the entry (⌘↩)" or writes-off text | :490 |
| `composer.editable` | boolean | `isEditable: model.canWrite` (LOG-10) | :417 |
| `composer.canSend` | boolean | `canSend` | :396-399 |
| `composer.sending` | boolean | `model.sending` | W/TicketDetailModel.swift:19 |
| `composer.canAddImages` | boolean | `canAddImages` | W/TicketDetailView.swift:400 |
| `composer.paperclipHelp` | string | paperclip `.help` (LOG-13) | :457-458 |
| `composer.status` | `Notice` \| null | status line priority: error, uploading, writes off (LOG-9) | :460-468 |
| `composer.maxIntakeFileBytes` | number | `ImageIntake.maxFileBytes` 40 MB (LOG-16) | W/ComposerTextView.swift:19 |

| Intent | Payload | Result | Swift |
|---|---|---|---|
| `retry` | — | null | "Retry" → `model.load()` (W/TicketDetailView.swift:43) |
| `setTab` | tab | null | `model.tab` segmented control |
| `copyRef` | — | null | `copyRef` — Rust writes the clipboard, `copied` for 2 s (W/TicketDetailModel.swift:98) |
| `assignToMe` | — | null | primary "Assign to me" → `list.assignToMe(ticket)` (W/TicketListModel.swift:221) |
| `reopen` | — | null | `list.applyFromHeader(.reopen, …)` (W/TicketListModel.swift:308) |
| `close` | — | null | after the dialog: `list.applyFromHeader(.close, …)` |
| `toggleTimer` | — | null | ⌘T on `TimerButton` → `timer.stop()` / `timer.start(ticket)` |
| `setDraft` | text | null | `list.drafts[key]` binding |
| `addImages` | `ImageIntakeItem[]` | null | `LogComposer.add` → `model.intake` (W/TicketDetailView.swift:402-405; W/TicketDetailModel.swift:73-93) |
| `removeImage` | imageId | null | `remove(_:)` + `clearError()` (W/TicketDetailView.swift:407-411) |
| `send` | — | boolean | `model.send(text, images:)` + clearing the sent key (W/TicketDetailView.swift:470-479; W/TicketDetailModel.swift:47-69) |
| `downloadAttachment` | attachmentId | null | `AttachmentsView.save` (W/RichTextView.swift:177-193); Save panel shown by Rust |

The Assign / Pending / Resolve entries of the primary button and More menu open the sheet models.
Rust reloads the detail when it opens and whenever the list's copy changes status, agent, team or
last update (DET-2), and checks the web stopwatch when a startable ticket opens (TMR-15).

### 5.6 `assign:<scope>`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `type` | `closed`\|`open` | `showAssign` | W/TicketDetailView.swift:11, 166 |
| `ticketId` | string | the sheet's `ticket` | W/AssignSheet.swift:8 |
| `title` | string | "Assign <ref>" / "Reassign <ref>" (ASG-6) | :23 |
| `subtitle` | string | ticket title | :25 |
| `assignToMe` | `{enabled, help}` | disabled busy or already yours; "It's already yours" (ASG-7) | :37-38 |
| `team` | `Dropdown` | teams, ticket's team preselected, placeholders (ASG-9) | :45-49, 108-117 |
| `agent` | `Dropdown` | agents of the team, "(you)", placeholders (ASG-10, ASG-11) | :50-55, 119-128 |
| `confirmLabel` | string | "Assign ticket" / "Reassign ticket" | :73 |
| `confirmEnabled` | boolean | `!(busy \|\| team == nil \|\| agent == nil)` (ASG-12) | :80 |
| `busy` | boolean | `list.working != nil` (spinner) | :18, 72 |
| `error` | string \| null | `error` | :16, 57-59 |

| Intent | Payload | Result | Swift |
|---|---|---|---|
| `open` | ticketId | null | `showAssign = true` + `.task loadTeams()` |
| `close` | — | null | Cancel / Esc |
| `assignToMe` | — | string \| null | `list.assignToMe` then dismiss or `error = notice.text` (W/AssignSheet.swift:28-33) |
| `setTeam` | teamId \| null | null | `team` + `onChange` clear agent, load agents (:87-90) |
| `setAgent` | agentId \| null | null | `agent` |
| `confirm` | — | string \| null | `list.assign(ticket, team:agent:)` (W/TicketListModel.swift:249-264) |

### 5.7 `pending:<scope>`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `type` | `closed`\|`open` | `showPending` | W/TicketDetailView.swift:12 |
| `ticketId`, `title`, `subtitle` | string | "Set <ref> to pending", ticket title (LC-9) | W/LifecycleSheets.swift:17 |
| `reason` | string | `reason` | :10 |
| `reasonRequired` | `RequiredMark` | `tried && empty` (LC-8) | :18 |
| `reasonError` | string \| null | "Enter why the ticket is waiting." (LC-10) | :21 |
| `timerLine` | string \| null | timer stop line (LC-11) | :22-28 |
| `error` | string \| null | `error` (LC-12) | :29 |
| `busy` | boolean | `list.working == ticket.id` | :32 |
| `confirmEnabled` | boolean | `list.working == nil` | :37 |

Intents: `open {ticketId}`, `close`, `setReason {reason}` (null), `confirm` → `string | null` =
`list.apply(.pending(reason:), to:)` (W/LifecycleSheets.swift:32-36; W/TicketListModel.swift:267). Rust
stops and saves the timer first (LC-4).

### 5.8 `resolve:<scope>`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `type` | `closed`\|`open` | `showResolve` | W/TicketDetailView.swift:13 |
| `ticketId`, `title`, `subtitle` | string | "Resolve <ref>" (LC-13) | W/LifecycleSheets.swift:64 |
| `service` | `Dropdown` | services, prefill, placeholders (LC-14, LC-15, LC-17) | :65-68, 114-136 |
| `serviceRequired` | `RequiredMark` | `tried && service == nil` | :65 |
| `subcategory` | `Dropdown` | subcategories (LC-16, LC-17) | :69-73, 138-148 |
| `subcategoryRequired` | `RequiredMark` | | :69 |
| `code` | `Dropdown` | `ResolutionCode.standard`, "Assistance" preselected (LC-18) | :54, 75 |
| `solution` | string | `solution` | :55 |
| `solutionRequired` | `RequiredMark` | | :76 |
| `solutionError` | string \| null | "Describe what fixed it. The caller sees this." (LC-19) | :79 |
| `timerLine` | string \| null | (LC-20) | :80-86 |
| `error` | string \| null | | :87 |
| `busy`, `confirmEnabled` | boolean | as Pending | :90, 96 |

Intents: `open`, `close`, `setService {serviceId}` (clears subcategory, loads; the prefill keeps it),
`setSubcategory`, `setCode`, `setSolution`, `confirm` → `string | null` = `list.apply(.resolve(r), to:)`
(W/LifecycleSheets.swift:90-95).

### 5.9 `newTicket:sheet`, `newTicket:window`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `type` | `closed`\|`open` | `workspace.newTicketStart` (sheet) / New Ticket window | W/Workspace.swift:21; W/App.swift:41-70 |
| `subtitle` | string \| null | "On <CI name>" (NEW-3) | W/NewTicketView.swift:68 |
| `ticketClass` | `TicketClass` | `draft.ticketClass` (NEW-5) | :69-75 |
| `organization` | `Dropdown` (searchable) | NEW-6 | :77-81 |
| `caller` | `Dropdown` (searchable) | NEW-7 | :82-87 |
| `service`, `subcategory` | `Dropdown` | NEW-8, NEW-9 | :88-101 |
| `impact`, `urgency` | `Dropdown` | `PickLists.impacts/urgencies` (NEW-10) | :102-111; K/NewTicket.swift:89-91 |
| `title`, `description` | string | `draft.title/description` (NEW-11) | :112-118 |
| `assignToMe` | boolean | `@AppStorage("newTicket.assignToMe")` (NEW-12) | :44, 119-121 |
| `required` | six `RequiredMark`s | `tried && …` (NEW-14) | :77-116 |
| `missingText` | string \| null | "Fill in the <field> first." | :122, 155-163 |
| `error` | string \| null | create error (NEW-16, NEW-17) | :123, 277-282 |
| `disabledText` | string \| null | creation off (NEW-13) | :124-127 |
| `creating` | boolean | spinner (NEW-18) | :58, 138 |
| `confirmEnabled` | boolean | `!(creating \|\| !canCreate)` | :139 |
| `showClearForm` | boolean | `workspace.newTicketDraft != nil` (NEW-23) | :129-132 |

| Intent | Payload | Result | Swift |
|---|---|---|---|
| `open` | — | null | plain start; restores a kept draft (NEW-22) (`load()`, :175-194) |
| `setClass` | ticketClass | null | segmented picker |
| `setOrganization` / `setCaller` / `setService` / `setSubcategory` / `setImpact` / `setUrgency` | id \| null | null | dropdown bindings + cascades (:143-152) |
| `setTitle` / `setDescription` | text | null | bindings |
| `setAssignToMe` | boolean | null | checkbox (persisted) |
| `clearForm` | — | null | `clear()` (:256-263) |
| `cancel` | — | null | Cancel / Esc: `keepDraft()` + dismiss (:133-137, 250-254) |
| `dismiss` | — | null | window close button: no draft (NEW-21) |
| `create` | — | string \| null | `create()` → `workspace.created` (:265-283; W/Workspace.swift:51-54; NEW-19, NEW-20) |

Org and CI starts are `shell.newTicketForOrg` / `newTicketForCI`; ⌘N in the main window is the native
menu (section 6); `menubar.newTicket` opens `newTicket:window`.

### 5.10 `timer`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `running` | `RunningTimerView` \| null: ticketId, ref, title, startedAt, elapsedSeconds, counter, clock | `TimerModel.running` + `TimeFormat.counter/clock` | W/TimerModel.swift:36; K/TimeTracking.swift:9-27, 59-78 |
| `saving` | boolean | `saving` (Retry and start disabled) | W/TimerModel.swift:41 |
| `notice` | `TimerNotice` \| null: text, isError, canRetry, dismissLabel | `notice` + × label "Discard this time"/"Dismiss" (TMR-11, TMR-20) | W/TimerModel.swift:24-29, 40; W/TicketListView.swift:202-219 |

Totals and web stopwatches are folded into rows (`RowTime`, `MenuRowRight`), the strip (`spent`) and
`timerButton` / `MenuRowButton`, so the UI never looks them up.

| Intent | Payload | Result | Swift |
|---|---|---|---|
| `start` | ticketId | null | `start(_:)` (W/TimerModel.swift:66) |
| `stop` | — | null | `stop()` (:88) |
| `toggle` | ticketId | null | `toggle(_:)` (:62) |
| `retry` | — | null | `retry()` (:102) |
| `clearNotice` | — | null | `clearNotice()` (discards a failed entry) (:57) |

### 5.11 `cmdb`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `query` | string | `query` | W/CMDBModel.swift:37 |
| `scope` | `{orgId, orgName}` \| null | `scope` bar (CMDB-7) | :61; W/CMDBView.swift:11-21 |
| `results` | `CIResults`: idle / searching / failed / empty / loaded {header, items} | `results` + `content` texts (CMDB-1…5) | W/CMDBModel.swift:20-25; W/CMDBView.swift:29-61 |
| `selectedId` | string \| null | `selectedID` | W/CMDBModel.swift:39 |
| `current` | `CIDetailView` \| null: id, key, className, name, orgName, headerLine, copied, webUrl, body | `current` + `CIDetailView` (CMDB-8…15) | W/CMDBModel.swift:47-49, 126; W/CMDBView.swift:74-235 |
| `placeholder` | string | "Select a config item to see its details." | W/ShellView.swift:61 |

`CIDetailBody.loaded`: fields, description, dependsOn/usedBy chips (with class-label help), usedBySentence
(locale list join, "is"/"are"), openTickets rows, openTicketsEmptyText (W/CMDBView.swift:155-234).

Intents: `setQuery {query}` (search with debounce, `search()`, W/CMDBModel.swift:75), `clearScope`
(:69), `select {id}` (`selectedID` didSet + `loadDetail`), `openCI {key, className, name}` (chip,
`open(_:)` :101), `retryDetail` (:106), `copyName` (Rust clipboard + `copied` 2 s, :118). All return null.

### 5.12 `orgs`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `query` | string | `query` | W/OrgsModel.swift:45 |
| `list` | `OrgList`: loading / failed / empty / loaded {orgs} | `orgs` + `visible` + list texts (ORG-1…5) | :44, 56-60; W/OrgsView.swift:115-150 |
| `selectedId` | string \| null | `selectedID` | :46 |
| `detail` | `OrgDetailView` \| null: id, name, code, headerLine, peopleLabel, configItemsLabel, ticketsHeading, tickets (`TableState`) | `OrgDetailView` (ORG-6…11) | W/OrgsView.swift:157-215 |
| `placeholder` | string | "Select an organization to see its open tickets." | W/ShellView.swift:73 |

Intents: `setQuery`, `select {id}` (+ `loadSelected`), `retryList` (`reloadList`), `retryTickets`
(`loadSelected`), `setTicketPage {page}` (`TicketTable.page`, W/OrgsView.swift:15). All null.

### 5.13 `people`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `query` | string | `query` | W/OrgsModel.swift:102 |
| `scope` | `{orgId, orgName}` \| null | `scope` (PPL-5) | :104 |
| `results` | `PeopleResults`: idle / searching / failed / empty / loaded {banner, people} | `results`, `total`, list texts (PPL-1…6) | :105-107; W/OrgsView.swift:242-281 |
| `selectedId` | string \| null | `selectedID` | :108 |
| `detail` | `PersonDetailView` \| null: id, displayName, initials, kindLine, organization, email, phone, teams, tab, assignedLabel, reportedLabel, tickets | `PersonDetailView` (PPL-7…14) | W/OrgsView.swift:284-364 |
| `placeholder` | string | "Select a person to see their details and tickets." | W/ShellView.swift:79 |

Intents: `setQuery`, `clearScope`, `select {id}` (`loadSelected()`), `setTab {tab}`,
`setTicketPage {page}`. All null.

### 5.14 `menubar`

| Field | Type | Mirrors | Source |
|---|---|---|---|
| `trayTitle` | string | `MenuBarStatus.title(now:)` (MB-2) | K/MenuBarStatus.swift:36-41 |
| `trayAccessibilityLabel` | string | `accessibilityLabel(now:)` (MB-5) | :43-52 |
| `query` | string | panel's own `@State query` (MB-7) | W/MenuBarView.swift:59 |
| `content` | `MenuContent`: loading / failed / empty / list {running, sections, height} | `content`, `sections`, `pinned`, scroll height (MB-8, MB-10, MB-11, MB-14, MB-15) | W/MenuBarView.swift:64-147 |
| `refreshing` | boolean | `mine.refreshing` | :213 |
| `updatedText` | string \| null | "Updated <relative>" (MB-16) | :217-219 |

`MenuRow`: ticketId, ref, title, needsYou, isRunning, right (running {counter} / spent / none; the panel's snapshot is re-sent every second while a timer runs, so the running row carries its live counter, MB-13), button (stop /
start {enabled, help} / slot) (MB-12, MB-13; W/MenuBarView.swift:156-206).

| Intent | Payload | Result | Swift |
|---|---|---|---|
| `setQuery` | query | null | panel search field |
| `openTicket` | ticketId | null | `openTicket(t)`: My tickets, search cleared, selected, main window forward (W/App.swift:102-110) |
| `openMain` | — | null | ⌘O / `macwindow` button: main window forward, selection kept |
| `newTicket` | — | null | ⌘N: open the New Ticket window (W/App.swift:99-101) |
| `refresh` | — | null | ⌘R: `mine.refresh()` |

Timer start/stop in the panel use the `timer` model; the timer notice in the panel is `timer.notice`.

## 6. Native side (Rust owns all of it unless marked React)

**Menus** (Tauri `Menu`, built and rebuilt by Rust when the session changes; KEY-1, KEY-2):

| Menu | Item | Shortcut | When | Action |
|---|---|---|---|---|
| App | standard (About, Hide, Quit …) | system | always | — |
| File | "New Ticket…" (replaces New) | ⌘N | signed in | main window forward + activate, open `newTicket:sheet` plain (NEW-1) |
| File | "Open iTop Wrap" | ⌘O | always | main window forward + activate |
| Edit | Undo, Redo, Cut, Copy, Paste, Select All | system | always | needed so the web view gets ⌘C/⌘V/⌘Z (LOG-3, LOG-14) |
| View | divider, "My tickets" … "People" | ⌘1 … ⌘5 | signed in | `shell.select` |
| Window | standard | system | always | the New Ticket window is not listed |

When the **menu bar panel** is the key window, ⌘N / ⌘O must do the panel's actions (`menubar.newTicket`
/ `menubar.openMain`, KEY-8) and ⌘R reloads My tickets: Rust routes the menu event by the focused
window's label.

**Keyboard handled in React** (web view `keydown`, then an intent): ⌘R in the main window when My or
Team tickets is shown → `list:<scope>.refresh` (LIST-64); ⌘R in the panel → `menubar.refresh`; ⌘T when
the detail's `timerButton` is `running` or an enabled `start` → `detail.toggleTimer` (KEY-4); ⌘↩ send /
confirm (KEY-5); Esc cancel / close dropdown first / close image viewer (KEY-6); Return on sign-in and
"Try again" (KEY-7); ↑/↓ list selection (KEY-9); Tab leaves the composer (KEY-10); dropdown keys
(KEY-11…17). React must `preventDefault` ⌘R so the web view never reloads.

**Tray** (MB-1…6): present only while signed in. Template icon (three stacked ticket stubs, 18×18)
followed by `menubar.trayTitle` in a monospaced-digit font, a11y `trayAccessibilityLabel`. Clicking
toggles the `menubar` window (borderless, 360 pt wide, under the icon, hides when it loses focus). The
slide animation (MB-4) is native; see Open points.

**Windows:**

| Label | Content | Rules |
|---|---|---|
| `main` | `index.html` | Title "iTop Wrap", single instance (SHELL-1). Size follows `session`: restoring and unreachable 480×320, signed out 480×520, both fixed; signed in resizable with the shell minimums (columns 180 + 300 + 380, height 520; SHELL-2). Signed in, closing hides it and the app keeps running; signed out, closing it quits (SHELL-16). Regular Dock app, activated at launch (SHELL-15). `dragDropEnabled: false` so drops reach the web view (LOG-15). |
| `menubar` | `menubar.html` | The panel. Created hidden; shown by the tray. |
| `new-ticket` | New ticket form for `newTicket:window` | Title "New Ticket", centred, opened only by `menubar.newTicket` (activates the app, main window stays closed), never restored, not in the Window menu. Rust closes it when `newTicket:window` becomes `closed`; its close button dispatches `dismiss` (no draft). Closes itself with no session (NEW-2, NEW-4). |

**Notifications:** none (NOTIF-1). The scaffold's notification plugin must not be used.

**Clipboard:** writes are Rust (`detail.copyRef`, `cmdb.copyName`, plain string, DET-10, CMDB-11).
Reads for pasting are Rust too: in the app the composer's `paste` handler prevents the default and
dispatches `detail.pasteFromClipboard`; Rust reads NSPasteboard like `ImageIntake.items(from:
preferText: true)` (image file URLs win, file URLs without images or any text → `"text"`, else PNG
then TIFF data) and feeds the intake; on `"text"` the UI inserts the pasted text itself (LOG-14).
The browser mock answers `"text"` and the UI takes images from the `paste` event.

**Locale:** every locale-dependent string (log dates, "Opened …", "since …", "Updated … ago", byte
counts, the CI list join) comes from `Native::format_*`: Foundation's formatters with the Mac's
current locale in the app (`native/mac/locale.rs`, `Date.RelativeFormatStyle`'s unit choice ported
onto NSCalendar), en_US in `FakeNative`.

**Main window hidden/shown:** closing the signed-in main window hides it and sends
`NativeEvent::MainWindowHidden`; showing it again sends `MainWindowShown`. The workspace treats them
like SwiftUI's views disappearing/appearing (Team poll, searches and detail loads stop; on show the
section appears again and the detail is made anew). My tickets' refresh, the timer and the menu bar
keep running.

**Open URL:** `open_url` (Rust) for rich-text links, outside-image chips, "Open in iTop on the web"
(`more.webUrl`, `CIDetailView.webUrl`, image-failure link), a person's mailto (DET-43, DET-46, DET-24).

**Dialogs:** Save panel for attachments is Rust (DET-33). The image picker is React
(`<input type="file" accept="image/*" multiple>`) feeding `addImages` (LOG-12). Confirmation dialogs
(sign out, close ticket) and the image viewer are React with texts from the snapshot.

**Timers, polling, persistence, Keychain, TLS:** Rust (POLL-1…10, PERS-1…12, AUTH-34…38).

## 7. Errors

`get_snapshot` and `dispatch` reject with an `AppError`:

| `type` | When |
|---|---|
| `notSignedIn` | a workspace model is addressed while signed out |
| `unknownModel` | the model id isn't one of section 3 |
| `invalidIntent` | the intent doesn't deserialize, or names a ticket/item/attachment the model doesn't have |
| `internal` | a bug; the UI logs it |

`{ type, message }`, serialized from a Rust `enum AppError` with the same serde attributes. Everything
iTop or validation can say is a business result: the exact texts of section 18 of the checklist are
built in Rust and arrive as result strings or snapshot fields.

## 8. Debug builds

`ITOPWRAP_WRITE_TICKETS`, `ITOPWRAP_WRITE_NEW`, `ITOPWRAP_SETTINGS`, `ITOPWRAP_TRACE`,
`ITOPWRAP_STDERR`, `ITOPWRAP_APPEARANCE` stay Rust/native concerns of debug builds (DBG-1…9): with writes
off the snapshots carry the writes-off texts and disabled flags, so the UI needs nothing extra.
`ITOPWRAP_DEMO` and its `_MANY`, `_STATE`, `_TIMEFAIL` switches become the TypeScript mock (section 9)
by design and are not ported to Rust. `ITOPWRAP_STDERR=<file>` appends stderr to the file and
`ITOPWRAP_APPEARANCE=light|dark` forces `NSApp.appearance` (`models::debug::native_switches`).

## 9. Mock backend

`ui/src/backend/mock/` implements the same `Backend` interface in memory with the Swift demo data
(DebugLaunch: user "Demo Agent", teams Network and Service desk, the wireframe tickets, CIs, orgs,
people, timer totals, the R-000405 web stopwatch, the R-000436 race, the R-000512 lookup, "fail" and
"refuse" triggers). It is a **UI fixture, not the source of truth**: rules are approximations. It is
used automatically in a plain browser and in tests. URL switches: `?state=empty|loading|error`,
`?signedOut=1`, `?unreachable=1`, `?timeFail=1`, `?many=1`, `?readOnly=1`, `?delay=<ms>`. Sign-in
passwords: `wrong` (invalid credentials), `rest` (REST profile refused), `offline` (can't reach), anything
else signs in. Images in the mock are `data:` URLs instead of `itop://` assets.

`ui/src/backend/contract.test.ts` checks that every snapshot is serde-shaped and runs the main flows
(sign in → list → select → detail → send entry; timer start/stop/save and failed save; image intake;
paging, org filter and ref lookup; resolve sheet and "Resolved just now"; new ticket created and assigned;
`useModel`).

## 10. Open points

1. **Tray slide animation (MB-4).** Tauri's tray title has no width animation and no Reduce Motion
   hook. Rust can approximate by drawing the title into the icon image per step (like Swift's
   `MenuBarIcon.label`) with `set_icon`, or accept no slide. Native owner to decide.
2. **Plugins not yet in `src-tauri/Cargo.toml`:** clipboard writes (tauri-plugin-clipboard-manager, or
   NSPasteboard through objc2) and the Save panel (tauri-plugin-dialog or rfd). The notification plugin
   and `notification:default` capability are present but must stay unused (NOTIF-1); removing them is
   cleaner.
3. **Window config conflicts.** `tauri.conf.json` gives `main` 1180×760 with min 900×560, which
   contradicts the 480×320 / 480×520 content-sized windows (AUTH-1, AUTH-9, SHELL-1) and the 860×520
   shell minimum. Rust must size the window per session state; the static minimums should go.
   `dragDropEnabled: false` is also needed on `main` (and on `new-ticket` is harmless).
4. **Option changes reload only the visible section.** That is what the Swift views do (`onChange` lives
   in the shown view), so a range change made in Organizations doesn't reload My tickets until its
   120 s poll. Mirrored here for parity; confirm this quirk is wanted.
5. **Locale-dependent strings.** Resolved: Foundation's formatters through `Native` (section 6,
   Locale). The mock uses en-GB.
6. **Pasting Finder-copied image files (LOG-14).** Resolved: `detail.pasteFromClipboard` (section 6,
   Clipboard).
7. **HEIC and other conversions (LOG-18).** Converting non-PNG/JPEG/GIF images needs Core Graphics /
   ImageIO from Rust (the `image` crate has no HEIC); owned by the Core B agent.
8. **ts-rs.** The design spec says TS types are generated from Rust with ts-rs. This hand-written
   `contract.ts` is the target shape; the Rust side should either generate identical types or add a test
   comparing its serialized fixtures to this file's types.
9. **Asset protocol and CSP.** `security.csp` is null today. If a CSP is added it must allow
   `img-src itop: data:`. The `itop://localhost/...` form is the macOS URL shape for Tauri custom
   protocols.
10. **Live values in rows.** Rows and buttons carry `{type:"running"}` and read the counter from `timer`
    to avoid re-emitting whole lists every second. If a screen ever needs the live value inside another
    model, add it there rather than computing it in React.
11. **`create`/`confirm` results on validation failure.** Swift returns nothing visible but the red
    labels; the contract returns `null` (nothing sent) with the marks in the snapshot. The UI must follow
    the snapshot's `open`/`closed`, never infer success from a `null` result.
