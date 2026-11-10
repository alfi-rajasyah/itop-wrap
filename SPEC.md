# Spec: iTop Wrap — native macOS client for iTop

Status: APPROVED 2026-11-10 — open questions still to answer before build

## Objective

A native macOS app that lets me do my daily iTop work without opening the iTop web UI.
It talks to my existing iTop server through the REST/JSON API (`/webservices/rest.php`).

**User:** me, an iTop user with a local username/password account and REST access.

**V1 user stories**
1. **Sign in** — I enter the server URL, username and password once; the app checks them (`core/check_credentials`) and remembers them (password in the macOS Keychain).
2. **My tickets** — I see a list of open `UserRequest` and `Incident` tickets assigned to me, with ref, title, status, priority, caller and last update. I can switch to "my team's tickets", search by ref or title, and refresh. If I type a full ref that isn't in my list (for example a colleague's ticket), the app fetches it from iTop and offers to open it in the app, as far as my iTop rights allow. Every ticket also has "Open in iTop on the web" for anything the app doesn't cover.
3. **Ticket detail** — I open a ticket to see its description (shown in full under the details row), public and private logs, caller, organization, team/agent and linked CIs.
4. **Create ticket** — I create a `UserRequest` or `Incident` with organization, caller, service and service subcategory (both required), title, description, impact and urgency (pick lists loaded from iTop; services are the ones the organization's contract covers, and subcategories are filtered by the chosen service).
5. **Update/resolve** — On a ticket I can add a public or private log entry, reassign (team + agent), and run lifecycle actions: assign, set to pending (a reason is required, saved to `pending_reason`), resolve (service and service subcategory, prefilled from the ticket and editable, both required; resolution code; and a solution that is required; no time field, matching the iTop "Mark as resolved" form), reopen, close — whichever ones are available for the ticket's current state.
6. **Browse CMDB** — I can search configuration items (`FunctionalCI` and subclasses such as Server, PC, ApplicationSolution) and `Person`s, see their main fields, and jump from a CI to its open tickets.
7. **Time spent** — On a ticket in the Assigned state I can start and stop a timer; tickets in other states show their total time but no timer. Only one timer runs at a time; starting another stops the first. The elapsed time is saved to iTop the way my iTop's time-tracking setup expects. The ticket list shows each ticket's total time and which one is running, the log composer can attach the timed minutes to an entry, and the resolve form is prefilled with the total, which I can correct.
8. **Menu bar** — A menu bar item shows how many tickets need me and the running timer. Clicking it opens a small panel: the running timer with a stop button, a search field, my tickets grouped by status (Assigned, Pending, Escalated TTO, Escalated TTR) with a start/stop timer button on each, and buttons for New ticket, Refresh and Open main window. The timer is the same one as in the main window. The item stays in the menu bar when the main window is closed.
9. **Organizations** — I can pick an organization (for example Org A) and see all of its open tickets from every agent, grouped by status (Assigned, Pending, Escalated TTO, Escalated TTR), with who each one is assigned to. From there I can jump to its people and CIs, or create a ticket for it. My own ticket list and the menu bar also have an organization filter that narrows my queue to one or more orgs.
10. **People** — I can search people and pick one to see their organization, email, phone and (for agents) teams, plus their open tickets in two tabs: "Assigned to them" (the default for agents) and "Reported by them" (the default for callers), grouped by status.

**Out of scope for V1:** offline mode, push notifications, attachments upload, editing CIs, SSO/LDAP/CAS, multiple servers, iOS.

**UX decisions (see UX-FLOWS.md):** "Needs you" means Escalated TTR assigned to me, or Escalated TTO unassigned in one of my teams. Lists are ordered escalated first. Offline is read-only. The only notification is "a ticket of mine newly needs me". The timer keeps counting through sleep and quit. New ticket stays organization first.

## Tech Stack

- Swift 6.3, SwiftUI, **macOS 26+** target, the newest SDK in Xcode 26.6 (MacOSX26.5). For your Mac only, so the newest APIs are fine.
- Swift Package Manager — no third-party dependencies
- `URLSession` + `Codable` for the REST client
- Security framework (Keychain) for the password
- REST API version `1.4` (your iTop is 3.2+; Task 0 confirms with `list_operations`); form-encoded POST with `auth_user`, `auth_pwd`, `json_data`

## Commands

```
Build:        swift build
Test:         swift test
Run (dev):    swift run ITopWrap
Package .app: ./scripts/bundle-app.sh        # builds release and wraps it into build/iTop Wrap.app
```

## Project Structure

```
Package.swift
Sources/
  ITopKit/           → REST client, request/response models, OQL helpers (no UI; fully unit-tested)
  ITopWrap/          → SwiftUI app: views, view models, Keychain storage
Tests/
  ITopKitTests/      → unit tests with a stubbed URLProtocol + recorded JSON fixtures
  ITopKitTests/Fixtures/ → sample iTop REST responses
scripts/
  bundle-app.sh      → packages the release build as a .app
SPEC.md
```

## Code Style

Swift API Design Guidelines, 4-space indent, `async/await`, value types for models, errors as typed enums.

```swift
public struct ITopClient: Sendable {
    let baseURL: URL
    let credentials: Credentials
    let session: URLSession

    /// Fetches objects with an OQL query, e.g. "SELECT UserRequest WHERE agent_id = :me".
    public func get<T: Decodable>(_ type: T.Type, class className: String,
                                  oql: String, fields: [String]) async throws -> [ITopObject<T>] {
        let body = Operation.get(className: className, key: oql, outputFields: fields)
        let response: ObjectsResponse<T> = try await send(body)
        return response.objects
    }
}

public enum ITopError: Error, Equatable {
    case invalidCredentials
    case server(code: Int, message: String)   // iTop "code" != 0
    case transport(String)
}
```

Conventions: one type per file in `ITopKit`; views own no networking — they call a view model, which calls `ITopClient`.

## Testing Strategy

- **XCTest/Swift Testing** in `Tests/ITopKitTests`, run with `swift test`.
- **Unit tests (ITopKit):** request encoding (form body, `json_data` shape), response decoding from fixtures, iTop error codes mapped to `ITopError`, OQL query builders. Target: every public `ITopClient` method has a test.
- **No network in tests** — a stub `URLProtocol` serves fixtures.
- **Manual smoke test** against the real iTop server for each user story before calling V1 done (read actions first; write actions on a test ticket only).
- UI is verified by running the app; no UI test suite in V1.

## Boundaries

- **Always:** run `swift test` before each commit; keep the password only in the Keychain; use HTTPS URLs (warn on `http://`); send write operations with a `comment` so changes are traceable in iTop history.
- **Ask first:** adding any dependency; changing the minimum macOS version.
- **Read-only against the real server (until you say otherwise):** during development the app only reads (`core/get`, `core/check_credentials`, `list_operations`). Every write (create, update, stimulus, TimeSpent) is built and tested against a recorded mock transport. Real writes are switched on only when you say so.
- **Never:** commit credentials, server URLs or real ticket data to the repo or fixtures; disable TLS validation; delete objects in iTop (`core/delete` is not used at all).

## Success Criteria

- [ ] `swift build` and `swift test` pass with zero warnings in `ITopKit`.
- [ ] Wrong password shows a clear "invalid credentials" error; right password lands on My Tickets, and relaunching the app needs no re-login.
- [ ] My Tickets shows the same open tickets as the iTop web UI's "My open requests" for my account.
- [ ] A ticket created in the app appears in the web UI with the same fields.
- [ ] A log entry, reassignment, and resolve done in the app show up in the ticket's history in the web UI.
- [ ] CMDB search finds a known server by name and shows its open tickets.
- [ ] `./scripts/bundle-app.sh` produces an `.app` that launches from Finder.

## Answers (2026-11-10)

| # | Question | Answer | What it changes |
|---|---|---|---|
| 1 | iTop version | 3.2 or newer | REST 1.4. Task 0 still confirms it with `list_operations`. |
| 2 | Customised data model | Some customisation; tickets are standard UserRequest and Incident | Task 0 reads one ticket with `output_fields: "*"` to list the real attributes before any form is built. |
| 3 | Where to write during development | Nowhere yet: read-only | All writes go to a mock transport (see Boundaries). |
| 4 | Minimum macOS | Newest only, for your Mac | macOS 26+ (newest SDK installed). |
| 5 | Time tracking | Combodo **Time Tracking** extension (itop-time-tracking 2.x, needs iTop 3.2+) | Time is stored as **TimeSpent** objects (user, start_date, end_date, duration, org, linked to the ticket). The stopwatch in iTop's web UI is the extension's own widget, not a stimulus, so the app keeps its own timer and creates one TimeSpent per stop with `core/create`. Only UserRequest and Incident are supported. Your account needs the "Time Tracker" profile. |
| 6 | Team membership readable | Yes | Read your Person's team links at sign-in; drives "needs you" and Team tickets. |
| 7 | Resolve fields | Service, subcategory, resolution code, solution only; solution is HTML | The app sends the solution as escaped HTML paragraphs (one `<p>` per paragraph). The sheet stays a plain text box in V1. |

## Server facts (Task 0 probe, 2026-11-10)

From a read-only probe; redacted samples are in `Tests/ITopKitTests/Fixtures/`, field list in `schema.json`.

- **REST 1.4**, 12 operations: the 8 `core/*` operations plus 5 from the TeemIp IPAM extension. `core/delete` exists but the app never uses it.
- **Your profiles** include REST Services User, Support Agent, Service Desk Agent, Time Tracker and Time Tracking Manager. The app needs no new rights.
- **Tickets** (UserRequest, Incident): standard fields. The app uses `ref`, `title`, `description`, `status`, `priority`, `impact`, `urgency`, `org_id`, `caller_id`, `team_id`, `agent_id`, `service_id`, `servicesubcategory_id`, `functionalcis_list`, `public_log`, `private_log`, `pending_reason`, `resolution_code`, `solution`, `tto`/`ttr` and `sla_tto_passed`/`sla_ttr_passed`. There is also a `time_spent` total on the ticket.
- **Teams:** `Person.team_list` lists `team_id` and `team_name`, with a role. "My teams" comes from there.
- **Time:** a `TimeSpent` has `start_date`, `end_date`, `duration` (seconds), `user_id` (your User), `contact_id` (your Person), `org_id`, `title`, `description`, `origin` (`calendar` seen) and `activity_id`. It has **no ticket field**: it points at an `Activity` (`activity_id`), and the Activity points at the ticket through `obj_class` (`UserRequest` or `Incident`) and `obj_id`. An Activity also has `label`, `background_color` and `text_color` (for the extension's calendar). Your open UserRequest already has one.
- **Writing time (later, once writes are on):** find the ticket's Activity with `SELECT Activity WHERE obj_class = :class AND obj_id = :id`. Then `core/create` a TimeSpent with `activity_id`, `user_id`, `contact_id`, `org_id`, `start_date`, `end_date`, `duration` (seconds) and a `title`.
- **CIs:** `FunctionalCI` returns the real class (`Server` seen) with `name`, `org_id`, `business_criticity`, `contacts_list`, `applicationsolution_list` and `tickets_list`. Relationships for the CI detail come from these lists and from `core/get_related`.

## Open Questions (remaining)

1. **TimeSpent ticket link.** Settled: TimeSpent → Activity (`activity_id`) → ticket (`obj_class`, `obj_id`). Still open: what to do when a ticket has no Activity yet. The proposal is that the app creates one, as a write, so it waits for writes to be switched on and gets tested on a ticket you choose. Earlier note: TimeSpent links to an `Activity` through `activity_id`, not to the ticket directly. Still to confirm with the second probe step: the Activity's fields (expected `obj_class`, `obj_id`), whether every ticket already has one, and whether the app has to create it before the first TimeSpent. Creating it would be a write, so it waits until you allow writes.
2. **Double counting.** If you also run the stopwatch in iTop's web UI, time can be logged twice. Proposal: the app shows a TimeSpent already running for you on that ticket and doesn't start its own.
3. **Long timers.** The extension auto-stops its own stopwatch after a maximum time (its docs say 4 h by default). Our decision 5 keeps counting through sleep and quit. Proposal: past 4 h the app asks "Still working on R-000412?" before saving.
