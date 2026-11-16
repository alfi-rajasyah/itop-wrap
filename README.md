# iTop Wrap

A macOS app for working iTop tickets without the web UI: your queue, ticket detail with logs and
images, the CMDB, organizations and people, plus log entries, assigning, pending and resolving, a time
tracker and a menu bar item. It talks to iTop through its REST API, and to GitHub only to check for a
newer release.

Not affiliated with Combodo. The app is written in Rust (Tauri) with a React UI built from the
[Beautiful UI](https://www.beautifului.dev/) components. It was first written in Swift; the rewrite's
design, the list of every behaviour it had to keep, and the contract between Rust and the UI are in
[docs/](docs/). The original spec, plan and design are [SPEC.md](SPEC.md), [PLAN.md](PLAN.md),
[DESIGN.md](DESIGN.md) and [UX-FLOWS.md](UX-FLOWS.md).

## Requirements

- macOS 13 or newer, Apple silicon. To build: Rust 1.96+, Node 24+ and the Xcode command line tools.
- iTop 3.2 or newer with REST 1.4.
- Your iTop account needs the **REST Services User** profile, and your usual agent profiles for the
  tickets you work. Time tracking needs Combodo's **Time Tracking** extension and the **Time Tracker**
  profile.

## Install

**From a release:** download `iTop-Wrap-<version>.dmg` from
[Releases](https://github.com/alfi-rajasyah/itop-wrap/releases), open it and drag **iTop Wrap** to
Applications.

The app isn't signed with a Developer ID or notarized by Apple, so the first time you open it macOS
says it can't check it for malicious software. Open it anyway: in Finder, Control-click iTop Wrap and
choose **Open**, or open it once, then go to System Settings > Privacy & Security and click
**Open Anyway**. You only do this once per version.

**From source:**

```bash
scripts/bundle-app.sh
```

This builds the UI and a release of the app, bundles and signs it, and puts it at
`build/iTop Wrap.app`. `scripts/make-dmg.sh` wraps that into `build/iTop-Wrap-<version>.dmg`.

If you have an Apple Development certificate (free: add your Apple ID in Xcode > Settings >
Accounts), the app is signed with it, so the Keychain's **Always Allow** lasts across rebuilds.
Without one it's signed ad hoc, and macOS treats each new build as a new app: the first time a build
opens it asks for your login keychain password. Choose **Always Allow**. `SIGN=adhoc` forces ad hoc;
`make-dmg.sh` always uses it, since a development signature carries your name and Apple ID email.

## Sign in

Enter your iTop address (for example `https://itop.example.com`), username and password. The password
is kept only in your macOS Keychain; the address and username are remembered so you don't retype
them. Relaunching signs you in again without asking. **Sign out** (the initials at the bottom of the
sidebar) removes the password from the Keychain.

## What it does

- **My tickets / Team tickets:** grouped by status, escalated first. A red dot marks the only tickets
  that need you: escalated TTR assigned to you, or escalated TTO unassigned in one of your teams.
  The View menu sets the date range, resolved and closed tickets, sorting and page size.
- **Ticket detail:** description with links and pasted images, attachments, public and private logs,
  linked CIs. The main action follows the state (Assign to me, Resolve ticket, Close ticket); the
  rest, Reopen included, is in the "…" menu. A ticket you resolve stays open so you can close it
  next, even when resolved tickets are hidden. Type a full ref in the search field to open a ticket outside your queue.
- **Changes:** log entries, assign and reassign, set to pending, resolve (service, subcategory,
  resolution code, solution), **Resolve and Close** in one step, reopen, close, and New ticket (⌘N), which assigns the ticket to you
  unless you untick **Assign to me**. **Edit…** (⌘E, in the ticket's "…" menu) changes the same
  fields as New ticket on an existing ticket and saves only what you changed; a description with
  images or formatting stays read-only (change it in iTop on the web), so saving never drops them.
  Every change carries the comment
  "iTop Wrap" in the ticket's history.
- **Images in log entries:** paste (⌘V), drop or pick (paperclip) up to 5 PNG, JPEG or GIF images,
  4 MB each, and check the thumbnails before sending. They're stored the way the web UI stores a
  pasted image, so they show inside the entry on the web too. Other image types (a HEIC photo, a
  TIFF screenshot) are converted. If iTop refuses an image, its message shows and your draft stays.
- **Time:** one timer (⌘T) on Assigned tickets of yours. Each stop saves a time entry like iTop's own
  stopwatch. Starting another timer stops and saves the first; pending and resolve stop it too; it
  keeps running if you quit. If your web stopwatch is already running on a ticket, the app shows it
  instead of starting a second one.
- **Menu bar:** your Needs you count and the running timer; the panel lists your tickets with start
  and stop buttons, and a Resolve button that opens the Resolve form in its own window, like New
  ticket. It keeps working with the window closed; ⌘O brings the window back.
- **CMDB, Organizations, People:** search CIs and see their relationships and open tickets; an
  organization's tickets from every agent; a person's assigned and reported tickets.

- **Updates:** iTop Wrap ▸ Check for Updates… asks GitHub for the latest release. The app also checks
  once a day by itself (release builds only; turn it off with **Check automatically** in that dialog)
  and shows a line above the account button when a newer version exists. It never downloads or
  installs anything: **Download** opens the release page, and you drag the new app to Applications as
  usual. Only the version number is read from GitHub's answer.

Release builds can change anything your iTop account may change. Debug builds (`tauri dev`) are
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
cargo test --workspace
```

```bash
npm --prefix ui run typecheck && npm --prefix ui test
```

```bash
ui/node_modules/.bin/tauri dev
```

- `crates/itop-kit` holds the REST client and every ticket rule, covered by tests with REST stubbed.
- `src-tauri` is the app: the models the UI shows, the menu bar item, menus, windows and the Keychain.
- `ui` is the React UI. It only renders what Rust sends and sends back what you do
  ([docs/rewrite/contract.md](docs/rewrite/contract.md)). Opened in a plain browser
  (`npm --prefix ui run dev`) it runs on made-up tickets, and nothing is sent to any server.

Debug builds read these switches (release builds ignore them):

| Switch | Effect |
|---|---|
| `ITOPWRAP_SETTINGS=<name>` | Keep the address, username and Keychain item apart from your real ones |
| `ITOPWRAP_WRITE_TICKETS=R-…,R-…` | Writes on for these tickets only; the client refuses any other write before sending |
| `ITOPWRAP_WRITE_NEW=1` | Also allow creating tickets, and writes on the ones created |
| `ITOPWRAP_TRACE=1` | Log REST calls and UI steps to stderr, never passwords or field values |
| `ITOPWRAP_STDERR=<file>` | Append stderr to a file (for a debug app opened from Finder) |
| `ITOPWRAP_APPEARANCE=light\|dark` | Force an appearance |

Two tests run against a real iTop and are off by default, because they create and close tickets. Point
them only at a test server:

```bash
ITOP_URL=… ITOP_USER=… ITOP_PASSWORD=… cargo test --workspace -- --ignored the_whole_round the_app_against
```

`scripts/probe-itop.py`, `scripts/probe-images.py` and `scripts/probe-stopwatch.py` are read-only
probes that save redacted summaries to `crates/itop-kit/tests/fixtures/`. Never commit credentials,
server addresses or real ticket data.

## License

[MIT](LICENSE). iTop is a trademark of Combodo; this app isn't affiliated with or endorsed by Combodo.
