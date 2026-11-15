# iTop Wrap: Rust rewrite with Beautiful UI

Status: approved in conversation 2026-11-16 (the owner delegated the open calls; they are marked "my call").

## Intent

Rewrite iTop Wrap from Swift/SwiftUI to Rust, with a UI built from the Beautiful UI components
(https://www.beautifului.dev/, MIT). Every behaviour stays the same; the look and feel get better.

## Goals

- Every behaviour in `docs/rewrite/parity-checklist.md` works in the new app.
- All rules (queries, lifecycle, timer, grouping, "needs you", errors) live in Rust and are covered by
  the ported tests, run against the same recorded iTop responses.
- The UI uses Beautiful UI components where one fits and the same visual language everywhere else.
- Verified against the owner's homelab iTop (3.2+, no Time Tracking extension), where writes are allowed.

## Non-goals

- New features. Windows or Linux builds. Signing with a Developer ID or notarizing.
- Merging to main, releasing or bumping the version: the owner does that.

## Anti-goals

- A behaviour silently dropped because the web view made it awkward.
- Rules duplicated in TypeScript and Rust, where they can drift.
- A password anywhere except the macOS Keychain (and `.env.local` for tests, never committed).

## Constraints

- Beautiful UI is React + Tailwind, so the UI cannot be Rust. It runs in a Tauri 2 web view.
- macOS only, Apple silicon. Bundle identifier and Keychain service name ("iTop Wrap") stay, so the new
  app replaces the old one and finds the saved password.
- The homelab has no Time Tracking extension: saving time entries is verified against the recorded
  responses and a stub server only.
- Debug builds stay read-only unless writes are switched on for named tickets, as today.

## Approaches

1. **Tauri 2: Rust core, React UI (picked).** The only way to use the components as written. Rust owns
   all behaviour; React renders.
2. **Pure Rust UI (Dioxus or egui), imitating the look.** All Rust, but every component is re-made by
   hand and it stops being "Beautiful UI". Rejected: slower and off-brief.
3. **Keep SwiftUI, move only ITopKit to Rust through FFI.** Keeps the native feel, but changes nothing
   the owner asked to change in the UI. Rejected.

## Architecture (my call)

```
crates/itop-kit/     Pure Rust library: the port of Sources/ITopKit. No Tauri, no UI.
                     reqwest + serde. Tests ported from Tests/ITopKitTests with the same fixtures.
src-tauri/           The app: models (ports of the Swift view models), Tauri commands, tray,
                     Keychain, notifications, settings. Tests ported from Tests/ITopWrapTests.
ui/                  Vite + React 19 + TypeScript + Tailwind 4. Vendored Beautiful UI in
                     ui/src/components/bui/. Renders snapshots, sends intents.
```

**Rust owns the models.** The Swift app has view models (`SessionModel`, `Workspace`,
`TicketListModel`, `TicketDetailModel`, `TimerModel`, `CMDBModel`, `OrgsModel`, `PeopleModel`,
`ListOptions`). Each is ported to Rust nearly one to one. The UI holds only what is purely visual
(which popover is open, text being typed before it is sent to the model). Reasons:

- The menu bar must keep working with the window closed, so the list, the timer, the 2-minute refresh
  and the notification cannot live in a web view.
- Parity is checked by Rust tests instead of by eye.

**How the UI talks to Rust.** Commands for intents (`ticket_list_refresh`, `timer_toggle`, ...), each
returning `Result<T, AppError>`. Models push a fresh snapshot to every window with a Tauri event
(`session`, `list:mine`, `list:team`, `timer`, ...) whenever they change. TypeScript types are generated
from the Rust types with `ts-rs`, so they cannot drift. A mock backend with canned snapshots lets the
UI be built and tested in a browser without Tauri (it replaces the Swift app's debug demo mode).

**Menu bar.** A Tauri tray icon with the count and timer as its title, and a borderless panel window
under it for the list. The "time slides out of the icon" animation is matched as closely as the tray
API allows.

**Native pieces.** Keychain through `security-framework`; notifications through the Tauri notification
plugin; settings in the same `UserDefaults` domain where feasible so the old app's remembered server,
user and view options carry over, otherwise migrated once on first launch.

## Design direction (my call)

Beautiful UI's look replaces the system-native look. Kept from DESIGN.md because they are meaning, not
style: teal means only "needs you"; status is always text, never colour alone; the ticket ref and
durations are monospaced with tabular figures; light and dark both work; every keyboard shortcut stays.

## How the work is split

1. Contract (orchestrator): shared types, command list, events, mock backend, scaffold.
2. Parallel, Opus agents in separate worktrees:
   - Core A: transport, errors, session, queries, detail, rich text, CMDB, orgs, people.
   - Core B: writes, lifecycle, new ticket, log images, time tracking, menu bar status.
   - UI A: shell, sidebar, login, ticket lists, view options, search.
   - UI B: ticket detail, composer, sheets, new ticket, CMDB, organizations, people, menu bar panel.
3. Orchestrator: the app models and commands, tray, timer wiring, notifications, shortcuts, packaging.
4. Verification: ported tests, live run against the homelab, checklist walk, independent review.
5. Swift sources removed once the hands-on check (docs/rewrite/hands-on-check.md) passes.

## Left to the builder

Crate choices beyond those named, file layout inside each crate, component composition, and test
structure, as long as the checklist and the contract hold.
