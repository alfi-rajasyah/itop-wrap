# iTop Wrap

A native macOS app for working iTop tickets without the web UI: your queue, ticket detail with logs and
images, the CMDB, organizations and people, plus log entries, assigning, pending and resolving, a time
tracker and a menu bar item. It talks to iTop through its REST API.

Not affiliated with Combodo. The spec, plan and design live in [SPEC.md](SPEC.md), [PLAN.md](PLAN.md),
[DESIGN.md](DESIGN.md) and [UX-FLOWS.md](UX-FLOWS.md).

## Requirements

- macOS 26 or newer, Apple silicon, with Xcode 26 (Swift 6.2+) to build.
- iTop 3.2 or newer with REST 1.4.
- Your iTop account needs the **REST Services User** profile, and your usual agent profiles for the
  tickets you work. Time tracking needs Combodo's **Time Tracking** extension and the **Time Tracker**
  profile.

## Install

```bash
scripts/bundle-app.sh
```

This builds a release, adds the icon and `Info.plist`, signs it ad hoc and puts it at
`build/iTop Wrap.app`. Drag it to Applications if you like, then open it from Finder.

The app isn't signed with a Developer ID, so macOS treats each new build as a new app: the first time a
build opens it asks to use the password saved in your Keychain. Choose **Always Allow**.

## Sign in

Enter your iTop address (for example `https://itop.example.com`), username and password. The password
is kept only in your macOS Keychain; the address and username are remembered so you don't retype
them. Relaunching signs you in again without asking. **Sign out** (the initials at the bottom of the
sidebar) removes the password from the Keychain.

## What it does

- **My tickets / Team tickets:** grouped by status, escalated first. A teal dot marks the only tickets
  that need you: escalated TTR assigned to you, or escalated TTO unassigned in one of your teams.
  The View menu sets the date range, resolved and closed tickets, sorting and page size.
- **Ticket detail:** description with links and pasted images, attachments, public and private logs,
  linked CIs. The main action follows the state (Assign to me, Resolve ticket, Reopen ticket); the
  rest is in the "…" menu. Type a full ref in the search field to open a ticket outside your queue.
- **Changes:** log entries, assign and reassign, set to pending, resolve (service, subcategory,
  resolution code, solution), reopen, close, and New ticket (⌘N). Every change carries the comment
  "iTop Wrap" in the ticket's history.
- **Time:** one timer (⌘T) on Assigned tickets of yours. Each stop saves a time entry like iTop's own
  stopwatch. Starting another timer stops and saves the first; pending and resolve stop it too; it
  keeps running if you quit. If your web stopwatch is already running on a ticket, the app shows it
  instead of starting a second one.
- **Menu bar:** your Needs you count and the running timer; the panel lists your tickets with start
  and stop buttons. It keeps working with the window closed; ⌘O brings the window back.
- **CMDB, Organizations, People:** search CIs and see their relationships and open tickets; an
  organization's tickets from every agent; a person's assigned and reported tickets.

Release builds can change anything your iTop account may change. Debug builds (`swift run`) are
read-only unless you switch writes on for named test tickets (see Development).

## Troubleshooting

- **"This Mac doesn't trust the server's certificate."** The app only talks to servers macOS trusts.
  If your iTop uses a company or self-signed certificate, ask your admin for it, open it in Keychain
  Access and set it to **Always Trust**, then sign in again. The app never skips the check.
- **"This account can't use the iTop REST API."** Your account is missing the REST Services User
  profile. Ask your iTop admin to add it, then sign in again.
- **"Invalid credentials."** Wrong username or password, or the account is disabled. If you sign in to
  the web UI through single sign-on, your iTop may not accept a password for REST at all; ask your admin
  for a local password or an application account.
- **"The server answered with HTTP 403" (or 404).** Check the address points at iTop's root (the page
  before `/pages/UI.php`), and that a proxy or firewall in front of iTop lets `/webservices/rest.php`
  through.
- **iTop refuses a change** (for example a resolution code or impact value your iTop has customised):
  the sheet stays open with iTop's own message and nothing is changed.
- **Times look off by hours:** iTop dates have no time zone; the app reads them in your Mac's zone.
  Set both to the same zone.

## Development

```bash
swift build
swift test
```

`ITopKit` holds the logic and is covered by tests (Swift Testing, REST stubbed); `ITopWrap` is the
SwiftUI app. Debug builds read these switches (release builds ignore them):

| Switch | Effect |
|---|---|
| `ITOPWRAP_DEMO=1` | Made-up user and tickets; nothing is sent to any server |
| `ITOPWRAP_SETTINGS=<name>` | Keep the address, username and Keychain item apart from your real ones |
| `ITOPWRAP_WRITE_TICKETS=R-…,R-…` | Writes on for these tickets only; the client refuses any other write before sending |
| `ITOPWRAP_WRITE_NEW=1` | Also allow creating tickets, and writes on the ones created |
| `ITOPWRAP_TRACE=1` | Log REST calls and UI steps to stderr, never passwords or field values |
| `ITOPWRAP_APPEARANCE=light\|dark` | Force an appearance |

`scripts/probe-itop.py`, `scripts/probe-images.py` and `scripts/probe-stopwatch.py` are read-only
probes that save redacted summaries to `Tests/ITopKitTests/Fixtures/`. Never commit credentials,
server addresses or real ticket data.
