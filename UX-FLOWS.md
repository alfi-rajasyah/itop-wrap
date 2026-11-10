# UX flows: iTop Wrap

Status: DRAFT, mapped against the wireframe canvas on 2026-11-10. Every flow lists its trigger, steps (with the screen each happens on), what "done" looks like, edge cases, and **gaps** where the wireframes don't yet answer the question. Proposed defaults are marked **Proposed** and need your decision (see "Decisions needed" at the end).

## The core loop

Most of the day is one loop: **notice → open → act → move on**.

1. **Notice** that something needs you: the menu bar count, a notification, or the My tickets list.
2. **Open** the ticket: from the menu bar, the list, search, or an org or person view.
3. **Act** on it: start the timer, add to the log, reassign, set to pending, or resolve.
4. **Move on:** the ticket leaves "needs you", the count drops, and the timer is saved.

Every flow below is a variation of that loop. The rule that runs through all of them: **the app never loses your work** (drafts, timer, unsent entries) and **never overwrites someone else's change in iTop**.

---

## F1. First launch and sign in

**Trigger:** the app opens for the first time, or after Sign out.

1. **Sign in:** enter the server URL, username and password. An `http://` URL shows the warning under the field.
2. **Sign in to iTop:** the app checks the credentials (`core/check_credentials`). While it checks, the button reads "Signing in…" and the fields lock.
3. **Success:** the app saves the password to the Keychain, loads your user, person and teams, and opens **My tickets**. The menu bar item appears.
4. **Failure:**
   - "Your username or password was rejected."
   - "Can't reach itop.example.com."
   - "This account can't use the REST API. Ask your iTop admin to give it the REST Services User profile."

**Edge cases:** iTop answers but with a version too old for the REST API; the server has a self-signed certificate.

**Gaps:**
- The wireframe has no "checking" state and only one generic error message. → **Proposed:** add the "Signing in…" state and the three specific errors above.
- No first-run hint for the menu bar. → **Proposed:** a one-time note under the sign-in success: "iTop Wrap also lives in your menu bar."

## F2. Morning triage

**Trigger:** you start work and glance at the menu bar, or open the app.

1. The **menu bar count** shows how many tickets need you, for example "3".
2. Open the panel. Tickets are grouped by status, and a teal dot marks the ones that need you.
3. Click a ticket to open it in the main window, or start its timer straight from the panel.
4. In the main window, work down **My tickets**: the teal dots first, escalated sections second.

**Done when** the teal count reaches 0, or the remaining tickets are consciously left for later.

**Gaps:**
- **What exactly counts as "needs you"** isn't defined. It decides the count, the dot and notifications. → **Proposed:**
  - A ticket assigned to you where the caller wrote the latest public entry, or that has no entry from you yet.
  - Or an unassigned ticket in one of your teams.
  - Or any ticket assigned to you that is escalated (TTO or TTR).
- **Order inside a section** isn't decided. → **Proposed:** escalated first, then priority, then the oldest update.
- **Section order:** you chose Assigned, Pending, Escalated TTO, Escalated TTR, which puts the most urgent tickets last. → Kept as you set it; flagged in decisions.

## F3. Work a ticket with the timer

**Trigger:** you start on an Assigned ticket.

1. **Start the timer** from the ticket header (⌘T) or the menu bar row. Any other running timer stops and saves.
2. **Work and log:** write entries in the composer. "Time for this entry" is prefilled from the timer.
3. **Finish** one of three ways:
   - **Set to pending** (reason required). The timer stops and saves; the ticket moves to Pending.
   - **Resolve** (service and subcategory prefilled and required, resolution code, solution required; no time field). The timer stops and saves; the ticket leaves your open list.
   - **Stop the timer** and leave the ticket Assigned.
4. The success note confirms what happened and links back to the ticket.

**Edge cases:**
- You quit the app with a timer running.
- The Mac sleeps for an hour while the timer runs.
- Someone else reassigns the ticket while your timer is running on it.

**Gaps:**
- **Sleep and quit:** decided: the timer keeps counting through sleep and quit, with no prompt (decision 5).
- **Ticket reassigned under you:** → **Proposed:** stop the timer, save its time, and show "R-000412 was reassigned to Agent 2. Your timer stopped and saved 0:42."
- **Where the time goes in iTop** depends on your time-tracking extension (SPEC open question 5). Still blocking.

## F4. Pick up an unassigned ticket

**Trigger:** you see a dashed "Assign to me" in My tickets, Team tickets or an org view, or an Escalated TTO ticket.

1. Click **Assign to me** on the row, in the ticket header, or at the top of the Assign sheet.
2. The ticket moves to Assigned under you, Resolve becomes the main action, and the timer button appears.

**Edge cases:** someone else took it a moment earlier; your team doesn't own that ticket's service.

**Gaps:**
- **Race with a colleague:** → **Proposed:** if iTop shows it was already assigned, say "Agent 2 took I-000088 a moment ago" and refresh the row. Don't take it from them.
- **Menu bar:** it has no "Assign to me" for unassigned tickets (UX issue 2 from the review). → **Proposed:** add the same dashed pill to menu bar rows.

## F5. Look up a ticket that isn't yours

**Trigger:** a colleague or caller mentions "R-000512".

1. Type the ref in the search field of the main window or the menu bar.
2. If it's not in your list, one result appears: "Open: R-000512 · title".
3. It opens in the main window under "Search result: not in your queue", with the normal detail view and actions as far as your iTop rights allow.

**Edge cases:** the ref doesn't exist; you have no rights to it; you type a partial ref.

**Gaps:**
- **Partial refs:** → **Proposed:** look the ticket up in iTop only once the ref is complete ("R-" plus 6 digits, or digits alone after you pause); before that, filter your own list.
- **No rights or not found:** → **Proposed:** "R-000512 doesn't exist, or your iTop account can't see it." iTop doesn't tell the two apart, so the app shouldn't guess.

## F6. Check an organization or a person

**Trigger:** "What's open for Org A?" or "What else has this caller reported?"

1. **Organizations** → pick Org A → its open tickets from every agent, grouped by status, with assignees.
2. **People** → pick a person → "Assigned to them" or "Reported by them".
3. Click any ticket to open it.

**Gaps:**
- **Large orgs:** a real org may have hundreds of open tickets. → **Proposed:** show the first 50 per status and a "Show all 212" link, plus a search field over the org's tickets.
- **From ticket to org:** the org name in the ticket details isn't a link yet. → **Proposed:** make the org and caller in the details row links to their Organizations and People views.

## F7. Create a ticket while someone is on the phone

**Trigger:** a caller rings or walks up.

1. **New ticket** (⌘N, from the main window or the menu bar).
2. Type the organization, then the caller, then pick the service and subcategory. Fill in title and description, and pick impact and urgency.
3. **Create ticket:** the success note shows the new ref and an "Open ticket" link.

**Edge cases:** the caller isn't in iTop yet; you don't know their organization; you close the sheet by accident.

**Gaps:**
- **Start from the caller:** decided: stays organization first (decision 6).
- **Accidental close:** → **Proposed:** keep the draft. Reopening New ticket offers "Continue the ticket you started at 10:42?"
- **After creating:** → **Proposed:** an option to assign it to yourself and start the timer in the same step, for walk-up fixes.

## F8. Things go wrong: session, network, server errors

**Trigger:** your password expires, Wi-Fi drops, or iTop rejects a change.

- **Session expired:** → **Proposed:** a sign-in sheet over the current screen ("Your session expired. Sign in again to keep working."). Drafts, the timer and the open ticket stay put; after signing in, the action you were doing is retried.
- **Offline:** → **Proposed:** keep showing the last loaded data with a bar: "Offline. Showing tickets from 10:42." Reading still works. Writes stay enabled but are kept in an outbox and sent when you're back online, with a count in the bar ("2 changes waiting to send"). The timer keeps running locally.
- **iTop rejects a change** (a missing required field, a workflow rule): show iTop's message in plain words beside the action, and keep everything you typed.

**Gap:** none of these states are drawn yet, apart from the list's error state. → Needs wireframes after the decisions below.

## F9. The ticket changed in iTop while you had it open

**Trigger:** a colleague adds a log entry, or the caller replies by email, while you're writing.

- **Proposed:**
  - **Before a change is sent,** the app checks the ticket's last update. If it changed since you opened it, show "This ticket was updated in iTop (new public entry from Caller 1). Review it before sending." with "Show changes" and "Send anyway" (for log entries only).
  - **Lifecycle actions** (resolve, pending, reassign) on a changed ticket always reload first.
  - **New entries from others** show at the bottom of the log with a "New" label, without moving your scroll position or clearing your draft.

## F10. Refresh and notifications

**Proposed:**
- **Refresh:** every 2 minutes while the main window or menu bar panel is open, every 10 minutes in the background, and ⌘R any time. The footer always shows "Updated 1 min ago".
- **Notifications** (macOS, off until you turn them on):
  - "R-000431 needs you: Printer offline in Org A" when a ticket newly needs you.
  - "R-000367 is about to breach TTR (15 min left)" for escalation warnings.
  - Clicking a notification opens the ticket.
- **Never notify** about your own changes, or about tickets that aren't yours.

---

## Gaps already found in the wireframe review

| # | Issue | Proposed fix |
|---|---|---|
| U1 | The menu bar org filter hides the running timer | A running ticket is always pinned at the top of the panel, whatever the filter |
| U2 | No Assign to me in the menu bar | The same dashed pill on unassigned rows |
| U3 | Escape on sheets can't be tested on the canvas | Check in the real app: Esc cancels, but asks first if the sheet has typed text |

## Decisions (2026-11-10)

| # | Decision | What it changes |
|---|---|---|
| 1 | **"Needs you" = Escalated TTR assigned to you, or Escalated TTO unassigned in your teams.** Nothing else turns teal. (Revised in review 002: TTO stops once someone owns the ticket, so "TTO and mine" never happens.) | The teal dot, the sidebar count and the menu bar count mark only those tickets; sample count is 2 (R-000367 TTR, I-000088 TTO). An unassigned TTO ticket also gets the dashed "Assign to me"; taking it clears the dot. Other unassigned tickets and caller replies don't turn teal. The main teal button appears only on those tickets. |
| 2 | **Offline: read-only.** | While offline, the last loaded data stays visible with "Offline. Showing tickets from 10:42." Every write action stays visible but says "You're offline" instead of acting; drafts and the timer are kept. No outbox. |
| 3 | **Notifications: only "ticket newly needs you".** | One macOS notification when a ticket newly matches decision 1 (your ticket hits TTR, or an unowned ticket in your team hits TTO): "R-000367 is escalated (TTR): Badge reader offline at the entrance". Opt-in, and clicking it opens the ticket. No SLA-warning or team notifications in V1. |
| 4 | **Escalated first.** | Every list (My tickets, Team tickets, menu bar, Organizations, People) is ordered Escalated TTR → Escalated TTO → Assigned → Pending. |
| 5 | **Timer keeps counting through sleep and quit.** | No prompt on wake. Correct the time by hand if needed (the per-entry time field in the composer is editable). |
| 6 | **New ticket stays organization first.** | Pick the organization, then the caller from that org, as drawn. |

F2's proposed "needs you" definition and F8's outbox are replaced by decisions 1 and 2 above.

## Drawn states (2026-11-10)

The four "when things go wrong" screens are on the canvas in a new row, "When things go wrong", on both the Light mode and Dark mode pages. Each loads the real main window or Sign in with the state switched on.

| State | What it shows | Clicked through |
|---|---|---|
| Signing in and REST error | The button reads "Signing in…" with a spinner (which stops under Reduce motion). The REST error box says "This account can't use the iTop REST API. Ask your iTop admin to add the REST Services User profile…". | Static; checked visually |
| Offline (read-only) | A bar under the toolbar: "Offline. Showing tickets from 10:42." Clicking Resolve, Assign to me or sending a log entry adds "… will work again when you're back online." The timer keeps running. | Yes |
| Session expired | The window dims, and a sheet says "Your session expired" with a password field, Sign in again and Sign out. Afterwards the ticket, draft and running timer are still there. | Yes |
| Ticket changed in iTop | Above the composer: "This ticket was updated in iTop: a new public entry from Caller 1 at 11:20." with Show changes and Send anyway. Show changes adds the entry to the log with a "New" label and keeps your draft. Sending without reviewing does nothing until you choose one. | Yes |

**UX issues found while clicking:**
1. **Offline feedback is far from the click:** the "… will work again when you're back online" message appears in the bar at the top, not next to the button you pressed. **Proposed:** also show a short tooltip-style note beside the button for 3 seconds.
2. **Session expired accepts an empty password:** the wireframe closes the sheet with nothing typed. **Proposed:** show "Enter your password" under the field, like the other required fields.
3. **Send without reviewing:** clicking send while the "ticket changed" banner is showing currently does nothing visible. **Proposed:** shake-free emphasis: the banner border turns red and the text reads "Review the new entry or choose Send anyway first."
