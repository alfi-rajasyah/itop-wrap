# Implementation Plan: iTop Wrap

Status: in progress. Implements [SPEC.md](SPEC.md). Tasks 0 to 5b done; Checkpoint A passed (2026-11-11).

## Overview

Build the native macOS iTop client in thin vertical slices. Each slice goes all the way from REST call to screen and leaves a working app. Read-only features ship first, so we can check them against your real server without risk. Features that write to iTop come later, behind a checkpoint where you approve each first real write.

## Architecture Decisions

- **Two-target Swift package.** `ITopKit` holds all REST and data logic and is unit-tested with fixtures. `ITopWrap` is SwiftUI only. Most bugs are in parsing iTop's JSON, and this split lets us test that without the UI.
- **A single REST entry point.** Every call is a form-encoded POST to `/webservices/rest.php?version=1.4` with `auth_user`, `auth_pwd` and `json_data`, through one `send(_:)` method that maps iTop's `code`/`message` to `ITopError`. The other methods are thin wrappers around `core/get`, `core/create`, `core/update`, `core/apply_stimulus` and `core/check_credentials`.
- **The current user comes from iTop.** On login we fetch `SELECT User WHERE login = :login` to get `contactid`, then the Person and their teams. "My tickets" and "my team's tickets" filter on those IDs instead of guessing.
- **The ticket lifecycle is a configurable map.** REST can't tell us which actions are allowed in a ticket's current state. We ship the standard iTop map (for example `assigned → ev_resolve`) in one file, so a customised data model only changes that file.
- **No third-party dependencies.** URLSession, Codable, Security (Keychain) and SwiftUI cover everything.

## Design

The design direction lives in [DESIGN.md](DESIGN.md) (calm work tool, ENERGY 1 / RHYTHM 1 / MOTION 1, one teal accent meaning "needs you", ticket ref as the identity motif). It is reviewed and locked in Task 0b. Every checkpoint runs the antislop Delivery Gate against the running app.

## Dependency Graph

```
Task 1  REST transport + test harness
   │
Task 2  Sign-in + current user + app shell
   │
   ├── Task 3  My Tickets list ──┬── Task 4  Ticket detail ──┬── Task 6  Add log entry
   │                             │                            ├── Task 7  Reassign
   │                             │                            └── Task 8  Lifecycle actions → Task 8b Timer → Task 8c Menu bar
   │                             └── Task 9  Create ticket
   └── Task 5  CMDB browse  (links to Task 3's filtered list)
                                     │
                         Task 10 Package .app → Task 11 Final pass
```

## Task List

### Phase 0: Unknowns (with you, before any code)

## Task 0: Answer the open questions and probe the server read-only

**Description:** The open questions are answered (SPEC, Answers). With your OK, run read-only calls only: `core/check_credentials`, `list_operations`, one ticket with `output_fields: "*"`, your Person with its team links, and one of your own TimeSpent records. Record the real attribute codes, then save redacted copies as test fixtures and as the mock transport for every write.

**Acceptance criteria:**
- [ ] REST version, ticket attributes, team links and TimeSpent attribute codes (and its ticket link) are recorded in SPEC.md
- [ ] Redacted fixtures exist with no real names, emails or URLs

**Verification:**
- [ ] Manual check: you confirm the fixtures contain no real data

**Dependencies:** None · **Files:** `SPEC.md`, `Tests/ITopKitTests/Fixtures/*.json` · **Scope:** S

## Task 0b: Design review

**Description:** Review DESIGN.md. Produce static HTML mockups (no app code) of the ticket list, ticket detail, sign in and the three sheets, in both light and dark mode, plus the three app icon concepts at 1024 px and 32 px. You choose one icon concept.

**Acceptance criteria:**
- [ ] You approve DESIGN.md (dials, accent, motif, copy, shortcuts)
- [x] You pick one icon concept; the others are dropped (2. Stacked stubs, 2026-11-10)
- [ ] Mockups pass the antislop Delivery Gate by code inspection (static, so no click-through) and the contrast checks in both modes

**Verification:** Manual check: you sign off on DESIGN.md and the chosen icon

**Dependencies:** None (can run alongside Task 0) · **Files:** `DESIGN.md`, `design/*.html` · **Scope:** M

### Phase 1: Foundation

## Task 1: Package skeleton and REST transport

**Description:** Create `Package.swift` with the `ITopKit`, `ITopWrap` and `ITopKitTests` targets. Implement `ITopClient.send(_:)`: form-encode the request, decode the generic response envelope, and map errors (non-zero iTop `code` → `.server`, auth failure → `.invalidCredentials`, URLSession failures → `.transport`). Add a stub `URLProtocol` for tests.

**Acceptance criteria:**
- [x] The request body has the correct `auth_user`, `auth_pwd` and URL-encoded `json_data` (including passwords with `&`, `=`, `+`, `%`)
- [x] iTop codes map to `ITopError`: 1 → `.invalidCredentials`, others → `.server(code:message:)`; also HTTP status, network, untrusted certificate and non-JSON answers
- [x] Non-HTTPS URLs are flagged (`ServerAddress.isSecure`); pasted iTop page or REST URLs are trimmed to the base
- [x] Writes are refused in code (`.writesDisabled`) unless the client is created with `allowsWrites: true`

Done 2026-11-10: `swift test` 20/20 passing, no warnings.

**Verification:** `swift build` · `swift test --filter TransportTests`

**Dependencies:** 0 · **Files:** `Package.swift`, `Sources/ITopKit/ITopClient.swift`, `ITopError.swift`, `RESTOperation.swift` (not `Operation`, which clashes with Foundation), `ServerAddress.swift`, `JSONValue.swift`, `Tests/ITopKitTests/TransportTests.swift`, `StubURLProtocol.swift` · **Scope:** M

## Task 2: Sign-in, current user and app shell

**Description:** Add a login screen (server URL, username, password) that calls `check_credentials`, saves the password to the Keychain and the URL and username to UserDefaults, then loads the current User, Person and teams. Add the sidebar app shell (My Tickets / Team Tickets / CMDB, all placeholders) and Sign out.

**Acceptance criteria:**
- [x] A wrong password shows "Invalid credentials"; the right one opens the shell (tests; your real sign-in still to confirm)
- [x] After relaunch you're signed in without re-entering anything (tests; real relaunch still to confirm)
- [x] Sign out removes the Keychain item (tests, including a real Keychain round trip)
- [x] An account without REST rights gets its own message (the iTop wording is a guess until seen on a real server)

**Verification:** `swift test --filter AuthTests` · manual: `swift run ITopWrap` against your server (read-only)

**Dependencies:** 1, 0b · **Files:** `Sources/ITopKit/CurrentUser.swift`, `KeychainStore.swift`, `SessionService.swift`, `OQL.swift` (moved into ITopKit so they're testable), `Sources/ITopWrap/App.swift`, `LoginView.swift`, `ShellView.swift`, `SessionModel.swift`, `Theme.swift`, `Tests/ITopKitTests/AuthTests.swift` · **Scope:** M

### Checkpoint A: Foundation (passed 2026-11-10)
- [x] `swift test` passes, no warnings in ITopKit (31 tests)
- [x] antislop Delivery Gate PASS on sign in and shell (anti-slop/checkpoint-a-2026-11-10.md)
- [x] Real sign-in works against your server
- [x] You review before Phase 2

### Phase 2: Read-only features

## Task 3: My Tickets and Team Tickets list

**Description:** Run `core/get` on `UserRequest` and `Incident`, filtered to open statuses and to `agent_id = me` (or `team_id IN myTeams`). Show a sortable table with ref, title, status, priority, caller and last update, plus search by ref or title and a refresh button.

**Acceptance criteria:**
- [x] The list matches the web UI's "My open requests" for your account (confirmed by you against the real server)
- [x] Search narrows the list; refresh (⌘R, and every 2 minutes) reloads it
- [x] A full ref not in the list is fetched with `core/get` and shown as "Search result: not in your queue"; a ref you can't see or that doesn't exist says so
- [x] Empty, loading and error states are shown
- [x] Grouped by status as in DESIGN.md (not the plan's original sortable table), with the teal "needs you" rule and the sidebar count

Done 2026-11-10 on `task-3-ticket-lists`: 47 tests; click-through in the debug demo. Not in this task: org filter (5b), time spent (8b), Assign to me (7), the full detail view (4).

**Verification:** `swift test --filter TicketQueryTests` (OQL builder and decoding) · manual comparison with the web UI

**Dependencies:** 2 · **Files:** `Sources/ITopKit/Ticket.swift`, `TicketQueries.swift`, `Sources/ITopWrap/TicketListView.swift`, `TicketListModel.swift`, `ShellView.swift` (now three columns), `Tests/ITopKitTests/TicketQueryTests.swift` · **Scope:** M

## Task 4: Ticket detail

**Description:** A detail pane showing description, caller, organisation, team and agent, and the public and private logs parsed into individual entries (author, date, text, with HTML stripped or rendered), plus linked CIs from `functionalcis_list`.

**Acceptance criteria:**
- [x] Log entries appear in the same order and with the same authors as in the web UI (confirmed by you on real tickets)
- [x] Linked CIs are listed and clickable (they open in iTop on the web for now, and in the CMDB view after Task 5)
- [x] The description and log HTML become plain text with line breaks; images show as "[Image]" and nothing is fetched from links inside the HTML

Done 2026-11-10 on `task-4-ticket-detail`: 63 tests; click-through in the debug demo (both logs, Copy ref, linked CI).

**Verification:** `swift test --filter CaseLogTests` · manual check on 3 real tickets

**Dependencies:** 3 · **Files:** `Sources/ITopKit/CaseLog.swift` (with `HTMLText`, `LogDate`), `TicketDetail.swift`, `Sources/ITopWrap/TicketDetailView.swift`, `TicketDetailModel.swift`, `Tests/ITopKitTests/CaseLogTests.swift` · **Scope:** M

## Task 5: CMDB browse

**Description:** Search `FunctionalCI` (with its real class shown) and `Person` by name. Selecting a CI shows its main fields and its open tickets, using `lnkFunctionalCIToTicket`.

**Acceptance criteria:**
- [x] Searching for a known server by name finds it (confirmed by you)
- [x] A CI's open tickets match the web UI (confirmed by you)
- [x] Relationships (Depends on, Used by) match iTop's impact analysis for that CI (confirmed by you; `core/get_related` works on your server)
- [x] A ticket's linked CI opens in Config items, and a CI's open ticket opens in the ticket list
- Moved: Person search goes to Task 5b with the People view, as the design has it. "New ticket on this CI" waits for Task 9 (it's a write).

Built 2026-11-10 on `task-5-cmdb`: 73 tests; click-through in the debug demo.

**Verification:** `swift test --filter CMDBQueryTests` · manual check

**Dependencies:** 2 (can run in parallel with 3–4) · **Files:** `Sources/ITopKit/ConfigItem.swift` (with `CMDBQueries`, `CMDBService`), `RESTOperation.swift` (`getRelated`, `limit`), `Sources/ITopWrap/CMDBView.swift`, `CMDBModel.swift`, `Tests/ITopKitTests/CMDBQueryTests.swift` · **Scope:** M

## Task 5b: Organizations and People views, org filter

**Description:** Add an Organizations item under CMDB in the sidebar. It lists organizations with their open-ticket count, and selecting one shows all of its open tickets from every agent (`SELECT UserRequest WHERE org_id = :id AND status NOT IN (closed, resolved)`, same for Incident), grouped by status, with the agent's initials on each row. Its header links to the org's people and CIs and has New ticket. Also add an organization filter (multi-select menu) to My tickets and the menu bar. Add a People view in the same layout: search `Person`, show org, email, phone and teams, and open tickets in "Assigned to them" (`agent_id`) and "Reported by them" (`caller_id`) tabs.

**Acceptance criteria:**
- [x] Org A's ticket list matches the web UI's open tickets for Org A (checked by you with the range menu)
- [x] Filtering My tickets to Org A shows only your Org A tickets; no matches shows "None of your tickets are for Org A" with a link to the org view (checked by you)
- [x] Only organizations your iTop account can see are listed (iTop applies your rights to every query)
- [x] People: search by name (each word in the first or last name), "People in Org A" from the org view, details with org link, email and phone, teams for agents, and "Assigned to them" / "Reported by them" tabs
- Changed: open-ticket counts per person aren't in the People list (two queries per person); they're on the detail tabs. Org counts load every open ticket you can see with only its org id. "New ticket" waits for Task 9; the menu bar's org filter for Task 8c.

- [x] Added after your review: a date range menu (open by default, opened in the last 7/30/90 days, include resolved and closed) and a sort menu (priority, last update, newest, oldest) on all four ticket lists, one setting remembered between launches
- [x] With the range menu, an organization's and a person's lists match the web UI, including resolved and closed tickets (checked by you)

- [x] After your second review: "Any time" works with resolved and closed; a "Show 10/20/30/50/100/All" menu with a pager on every ticket list; People search says "Showing 100 of N"; long tables build rows lazily
- [x] A debug trace (`ITOPWRAP_TRACE=1`) showed iTop returns every ticket (556 for one org, 773 for one agent), so the "only 5" limit isn't on the server side
- [x] The "only 5" display: not a bug. A capture of your window showed one agent with 777 tickets: 5 open (Assigned) and 772 closed. Open-only lists showed the 5; "All tickets, any time" shows all 777
- [x] "Reentrant operation in its NSTableView delegate" warnings: selecting a row swapped out another column inside AppKit's selection callback. All five selectable lists now apply the selection one run-loop turn later (`Binding.deferred`). Gone in your terminal run with the fix

Built 2026-11-10 on `task-5b-orgs-people`: 102 tests; click-through in the debug demo.

**Verification:** `swift test --filter OrgQueryTests` · manual comparison with the web UI

**Dependencies:** 3, 4 · **Files:** `Sources/ITopKit/Organization.swift` (with `OrgQueries`, `OrgService`, `Person`), `Sources/ITopWrap/OrgsView.swift` (orgs, people, shared ticket table), `OrgsModel.swift`, org filter in `TicketListView.swift`, `Tests/ITopKitTests/OrgQueryTests.swift` · **Scope:** L

## Task 5c: Inline images, clickable links and attachments

**Description:** Show images from descriptions and log entries inline instead of "[Image]", make links clickable, and list the ticket's attachments. Images stored in your iTop are downloaded from your own server with your login. Images hosted anywhere else are never loaded automatically: they show as "Image from <host>" with a button that opens them in your browser. Clicking an inline image opens it full size. Links open in your browser.

**First, a read-only probe:** on one ticket that has a pasted image, check whether iTop's image links (`ajax.document.php?operation=download_inlineimage…`) accept the REST login, or need the web login (a session cookie). The approach depends on the answer.

**Acceptance criteria:**
- [x] Images stored in your iTop show inline, sized to the pane, in the description and in log entries (seen by you on a real ticket)
- [x] No request goes to any host other than your iTop server unless you click "Open in browser" (images from other hosts are only a placeholder; tested)
- [x] Links in descriptions and log entries are clickable and open in your browser (web and email links only; `javascript:` and `file:` stay plain text)
- [ ] The ticket's attachments are listed by name; each downloads on click into a Save panel, which shows the size once saved (your check: the Save panel can't be driven in the background). iTop can't send a size without the whole file, so the list has names only
- [x] An image that fails to load shows "Couldn't load this image" plus "Open in iTop on the web"

- [x] After your check: the full-size viewer fits the whole image to the window (with "Actual size" for 1:1), and outside images stay in the text instead of a big "Image from…" box: signature-size ones as a grey photo symbol, larger ones (logos, banners) as a one-line "🖼 host ↗" chip that opens them in the browser
- Attachments: not tested on real data, at your choice

Probe result (2026-11-11): your iTop answers the web image links with 403 to HTTP Basic auth, and serves `InlineImage` contents through REST, so images load through REST only. Built on `task-5c-images-links`: 117 tests.

**Verification:** `swift test --filter RichTextTests` (image and link extraction, host check) · manual check on a real ticket with a pasted image and an attachment

**Dependencies:** 4 · **Files:** `Sources/ITopKit/RichText.swift` (with `InlineImageLoader`, attachments), `CaseLog.swift` (shared `normalize`, entry HTML kept), `Sources/ITopWrap/RichTextView.swift`, `TicketDetailView.swift`, `scripts/probe-images.py`, `Tests/ITopKitTests/RichTextTests.swift` · **Scope:** M

### Checkpoint B: Read-only parity
- [ ] antislop Delivery Gate PASS on list, detail and CMDB, including empty, loading and error states
- [ ] Every read in SPEC user stories 2, 3, 6, 9 and 10 works on your real data
- [ ] You review before any write feature is built

### Phase 3: Write features (built against the mock transport; no real writes until you say so)

## Task 6: Add a log entry

**Description:** On ticket detail, a composer for public or private log entries, saved with `core/update` on `public_log` / `private_log` plus a `comment`.

**Acceptance criteria:**
- [ ] The entry appears in the web UI history with your name
- [ ] Failures show an error and keep the draft

**Verification:** `swift test --filter UpdateTests` (encoded request) · manual on a test ticket

**Dependencies:** 4 · **Files:** `Sources/ITopKit/TicketWrites.swift`, `Sources/ITopWrap/LogComposerView.swift` · **Scope:** S

## Task 7: Reassign team and agent

**Description:** Team and agent pickers (agents filtered by the chosen team) that call `apply_stimulus` with `ev_assign` / `ev_reassign` and the new `team_id` / `agent_id`.

**Acceptance criteria:**
- [ ] Reassigning updates the ticket in iTop and refreshes the list
- [ ] Only agents in the chosen team are offered
- [ ] "Assign to me" (row pill, ticket header, top of the Assign sheet) assigns the ticket to your own team and agent in one call, and the ticket moves to Assigned

**Verification:** `swift test --filter StimulusTests` · manual on a test ticket

**Dependencies:** 4 · **Files:** `TicketWrites.swift`, `Sources/ITopWrap/AssignSheet.swift` · **Scope:** S

## Task 8: Lifecycle actions (pending, resolve, reopen, close)

**Description:** Add `TicketLifecycle.swift` with a state-to-actions map for UserRequest and Incident. The detail view shows only valid actions. Set to pending asks for a reason (required, sent as `pending_reason` with `ev_pending`). Resolve asks for service and service subcategory (prefilled from the ticket, required, subcategory filtered by service), a resolution code and a solution (required), sent as `service_id`, `servicesubcategory_id`, `resolution_code` and `solution` with `ev_resolve`. Both block the action and show an inline error while the required field is empty, and both stop a running timer and save its time.

**Acceptance criteria:**
- [ ] Pending, resolve, reopen and close work on a test ticket and show in the history
- [ ] Pending without a reason, or resolve without a solution, is blocked before anything is sent
- [ ] Invalid actions for the current state are not offered
- [ ] iTop rejections (for example a missing required field) show iTop's message

**Verification:** `swift test --filter LifecycleTests` · manual on a test ticket

**Dependencies:** 7 (shares the stimulus code) · **Files:** `Sources/ITopKit/TicketLifecycle.swift`, `TicketWrites.swift`, `Sources/ITopWrap/ResolveSheet.swift`, `TicketActionsView.swift` · **Scope:** M

## Task 8b: Time spent timer

**Description:** Add a start/stop timer on ticket detail (with ⌘T), a total and a running marker on list rows, a "Time for this entry" field in the log composer. The Resolve sheet has no time field: resolving stops the timer and saves its time like any other stop. Only one timer runs at a time, and a running timer survives an app restart. Each stop creates one **TimeSpent** (Combodo Time Tracking) on the ticket's Activity (looked up by `obj_class` and `obj_id`), with start, end, duration in seconds, user, contact and org. If a TimeSpent is already running for you on that ticket from the web stopwatch, the app shows it instead of starting a second one. Past 4 hours it asks before saving.

**Acceptance criteria:**
- [ ] Stopping sends one correct TimeSpent `core/create` to the mock transport (real server only after you switch writes on)
- [ ] Starting a second timer stops and saves the first
- [ ] The start button appears only on Assigned tickets; a ticket that leaves Assigned (pending, resolved, escalated) stops its timer and saves the time
- [ ] Quitting and relaunching keeps a running timer

**Verification:** `swift test --filter TimerTests` (elapsed maths, single-timer rule, persistence, encoded write) · manual on a test ticket

**Dependencies:** 6, 8, and the TimeSpent codes from Task 0 · **Files:** `Sources/ITopKit/TimeTracking.swift`, `Sources/ITopWrap/TimerModel.swift`, `TimerButton.swift`, `LogComposerView.swift` · **Scope:** M

## Task 8c: Menu bar item

**Description:** Add a `MenuBarExtra` with a monochrome Stacked stubs icon, the Needs you count and the running timer. Its panel shows the running timer with stop, search, tickets grouped by status (Assigned, Pending, Escalated TTO, Escalated TTR) with per-row start/stop, plus New ticket, Refresh and Open main window. It shares one timer model with the main window, and stays running when the window is closed.

**Acceptance criteria:**
- [ ] Starting or stopping a timer in the menu bar is reflected in the main window at once, and the other way round
- [ ] The count matches My tickets' Needs you count
- [ ] Closing the main window leaves the menu bar item working; ⌘O reopens the window

**Verification:** `swift test --filter TimerTests` (shared model) · manual with the window open and closed · VoiceOver reads the item as "iTop Wrap: 2 tickets need you, timer 0:12 on R-000412"

**Dependencies:** 3, 8b · **Files:** `Sources/ITopWrap/MenuBarView.swift`, `App.swift`, `TimerModel.swift`, `Resources/MenuBarIcon.pdf` · **Scope:** M

## Task 9: Create a ticket

**Description:** A New Ticket form (UserRequest or Incident) with searchable dropdowns (type to filter, or open the full list) for organisation and caller (caller filtered by organisation, searched with `core/get` on `Person` as you type), service (`service_id`, required, limited to services the organisation's contract covers) and service subcategory (`servicesubcategory_id`, required, filtered by service), impact and urgency, a title and a description. Submits with `core/create` and opens the new ticket.

**Acceptance criteria:**
- [ ] The created ticket shows the same fields in the web UI
- [ ] Required fields are validated before sending
- [ ] Pick lists come from iTop, not hard-coded

**Verification:** `swift test --filter CreateTests` · manual: create one test ticket

**Dependencies:** 3 (opens in Task 4's detail view) · **Files:** `TicketWrites.swift`, `Sources/ITopKit/PickLists.swift`, `Sources/ITopWrap/NewTicketView.swift`, `NewTicketModel.swift` · **Scope:** M

### Checkpoint C: Write features
- [ ] antislop Delivery Gate PASS on every sheet and action
- [ ] Every write in SPEC user stories 4, 5, 7 and 8 is verified in the web UI history
- [ ] No test data is left that you didn't want

### Phase 4: Ship

## Task 10: Package as a .app

**Description:** `scripts/bundle-app.sh` runs a release build, wraps it with an `Info.plist` and an icon into `build/iTop Wrap.app`, and ad-hoc signs it.

**Acceptance criteria:** [ ] The app launches from Finder and keeps the Keychain login

**Verification:** run the script, then open the app from Finder

**Dependencies:** 2 · **Files:** `scripts/bundle-app.sh`, `Resources/Info.plist`, `Resources/AppIcon.icns` (Stacked stubs, see DESIGN.md) · **Scope:** S

## Task 11: Final pass

**Description:** Go through the SPEC success criteria, get to zero warnings, and write a short README (install, sign in, troubleshooting TLS and REST access).

**Acceptance criteria:** [ ] All SPEC success criteria are checked off

**Verification:** `swift build -c release` · `swift test` · run through the success criteria

**Dependencies:** all · **Files:** `README.md`, `SPEC.md` · **Scope:** S

### Checkpoint D: Complete
- [ ] All SPEC success criteria are met, and you sign off

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Customised data model (extra required fields, renamed states or stimuli) | High | Task 0 asks up front; lifecycle and field lists live in one config file each |
| Self-signed or internal TLS certificate | Med | Never disable TLS. README explains trusting the CA in Keychain Access |
| Case logs come back as HTML with inconsistent formatting | Med | Fixture-driven `CaseLogTests` in Task 4; fall back to plain text |
| REST can't list the actions allowed per state | Med | Static lifecycle map plus iTop's rejection message shown to you (Task 8) |
| Slow `core/get` on big ticket tables | Low | Request only the fields we show; limit to open statuses |
| Swift 6 strict-concurrency friction in the UI | Low | `@MainActor` view models, `Sendable` client |
| Org counts load every open ticket you can see (org id only) | Low | One light query per ticket class; if it gets slow on a big iTop, count only on demand |
| Account lacks the "Time Tracker" profile, or TimeSpent isn't writable over REST | Med | Task 0 checks read access; the first real write test (when you allow it) is one TimeSpent on your own ticket. If it fails, the timer keeps time locally and shows "Not saved to iTop" |
| Time logged twice (app timer and web stopwatch) | Med | The app detects a running TimeSpent for you and shows it instead of starting its own |

## Parallelization

The plan runs sequentially by default. If you want to go faster later, Task 5 (CMDB) can run alongside Tasks 3–4 once Task 2 is done, and Task 10 can run any time after Task 2.

## Open Questions

Answered on 2026-11-10 (see SPEC, Answers). Still open, settled by Task 0's read-only probe or a quick decision:
1. TimeSpent attribute codes and how it links to the ticket
2. Avoiding double counting with the web stopwatch (proposal in SPEC)
3. What happens past 4 hours (proposal in SPEC)
