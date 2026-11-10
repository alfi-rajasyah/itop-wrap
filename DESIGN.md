# DESIGN.md: iTop Wrap

Status: DRAFT, awaiting review in PLAN.md Task 0b. Filtered with antislop (mode: during).

## Design Read

Reading this as: a native macOS work tool for an IT agent triaging their ITSM queue for hours at a time, in a calm, platform-native style. Dial: ENERGY 1 / RHYTHM 1 / MOTION 1.

- **ENERGY 1:** the app is opened dozens of times a day to answer "what needs me?". Loud chrome would get in the way of reading tickets.
- **RHYTHM 1:** every list and detail pane has the same structure, on purpose, so the eye knows where the ref, the status and the action are without searching.
- **MOTION 1:** only hover, selection and the system's own sheet and split-view transitions. Motion would slow down rapid keyboard triage.

## Identity

| Element | Decision | Reason (R-31) |
|---|---|---|
| Platform | SwiftUI system controls, `NavigationSplitView`, system window chrome | Native behaviour (keyboard, VoiceOver, Dynamic Type, dark mode) comes for free and feels at home next to your other Mac apps |
| Accent | One fixed teal. Light: `#0F6E56` text, `#E1F5EE` tint. Dark: `#5DCAA5` text, `#085041` tint | Calm, distinct from iTop's web orange and from default system blue; used only for "needs you" (see below) |
| Danger | System red, only for priority 1 and iTop errors | Real urgency is the only reason to break the calm |
| Neutrals | System label/secondaryLabel/separator colours | Follow the user's appearance settings with no custom greys to drift |
| Typeface | SF Pro (system) for all text | Built for screen legibility at small sizes, supports Dynamic Type; a custom face adds no value to a triage tool |
| Identity motif | The ticket ref (`R-000412`, `I-000088`) in SF Mono, small, tabular figures, the same weight everywhere: list rows, detail header, window title, log mentions. Click a ref to copy it | Agents talk in refs ("can you look at R-412?"); making the ref the one consistent typographic voice ties every screen to the job |
| Icons | SF Symbols chosen per meaning: `tray.full` my tickets, `person.2` team tickets, `server.rack` config items, `person.crop.rectangle.stack` people, `square.and.pencil` new ticket | Native and relevant to each item; no generic decorative glyphs |
| People | Initials in a neutral circle | No invented photos (R-23) |
| Shadows / radius / blur | System defaults only; no custom shadows, glass or glow | Elevation is already handled by the window, sidebar and sheets |
| Theme | Follows system light/dark; both verified at every checkpoint. The wireframe canvas has a Dark mode page with a dark copy of every screen | An all-day tool must respect the user's setting (R-21, R-34) |

**Palette count:** system neutrals + 1 accent (teal) + 1 semantic danger (red), within R-29.

**Contrast (checked, WCAG AA):** teal on white 6.2:1 · white on teal 6.2:1 · teal on teal tint 5.46:1 · dark teal text on near-black 8.3:1 · dark tint pill 6.31:1.

## How the accent earns its place

The teal appears in exactly one meaning: **this ticket needs you**, which means it is escalated **TTR and assigned to you**, or escalated **TTO, unassigned, in one of your teams**. (TTO only escalates while nobody owns the ticket, so "TTO and mine" can never happen.) Nothing else turns teal. An unassigned TTO ticket shows both the teal dot and the dashed "Assign to me" pill; assigning it to yourself clears the dot.

- The sidebar count on "My tickets" counts only those tickets.
- Their list rows get a teal "Needs you" pill.
- The detail view shows the primary action as the one filled (teal) button.

Sidebar selection uses the macOS accent colour the user chose in System Settings (blue by default); the app doesn't override system controls, and teal stays reserved for "needs you".

Every other status pill (Assigned, Pending, Resolved, Closed) is neutral text on a neutral tint. Status is always written as text, never shown by colour alone.

## Time spent (start/stop timer)

| Place | Design | Reason (R-31) |
|---|---|---|
| Ticket detail header | An icon-only stopwatch button (SF Symbol `stopwatch`) next to the main action. While running it shows a stop icon plus a live `00:12:34` counter. Its tooltip and accessible name are "Start timer" / "Stop timer" | Starting work and stopping it are the two moments you're looking at the ticket; an icon keeps the header quiet next to the main action |
| Ticket list rows | Each row shows its total (`1h 20m`). The running ticket shows a small stopwatch icon and `0:12` in bold | You can see where your time went and which ticket is running without opening it |
| Log composer | A "Time for this entry" field, prefilled from the running timer and editable | Ties the time to the work it describes |
| Resolve sheet | No time field. Resolving stops the timer and saves its time | Matches iTop's own "Mark as resolved" form: service, subcategory, resolution code, solution |

- **Assigned tickets only.** The start button appears only on tickets in the Assigned state, in the main window and the menu bar. Other tickets still show their total time, and the menu bar keeps an empty slot so the times line up. When a ticket leaves Assigned, its running timer stops and the time is saved.
- **One timer at a time.** Starting a timer on another ticket stops the first and saves its time. Quitting the app doesn't lose a running timer: it keeps running and shows on relaunch.
- **No new colour.** The timer is neutral. The running state is shown by the icon changing (stopwatch to stop), a live counter and bold figures, never colour alone, so teal keeps its one meaning.
- **The motif extends to durations.** Times use the same SF Mono tabular figures as refs, so a ticket's ref and its time read as one family. The counter changes digits only, with no animation.
- **Keyboard:** ⌘T starts or stops the timer on the selected ticket.
- How time is saved to iTop depends on your setup (SPEC open question 5).

## Icons over text

Text stays where the words are the content or a mistake is costly. Everything else uses an icon or is removed. Every icon-only control keeps a tooltip and an accessible name (R-39).

- **Removed as repeated:** status labels in list rows (the group header says it), the "Needs you" label on list rows (a teal dot instead), keyboard hint lines in sheets and the composer (in tooltips instead).
- **Hints behind ⓘ:** help lines in sheets ("Only members of the selected team are listed" and similar) become an ⓘ button whose tooltip and accessible name hold the text.
- **Icons:** field labels in the detail strip (person = caller, building = organization, two people = team, server = linked CI, stopwatch = time spent); ticket type in the detail header (speech bubble with ? = user request, warning triangle = incident); priority (`!` in red for P1, grey dot for P2, nothing visible for P3–P4); send icon for "Add to log"; stopwatch in the time field; initials avatar for the account (menu with Sign out); class icons in CMDB results.
- **Assignee:** the detail strip shows the agent as its own item, an initials avatar plus the name ("You", "Agent 2"). "Unassigned" is bold, with a dashed empty circle, so it stands out. List rows end with the agent's initials when the ticket is assigned to someone else, or a dashed circle when it's unassigned. Your own tickets show nothing, since that's the normal case in My tickets.
- **Kept as text:** ticket titles, refs, caller names, the main action button, Public/Private log (a lock icon is added to private entries; the Private tab is text only, because macOS segmented controls don't show icons), status in the detail header, empty and error messages, sheet buttons.

## Menu bar item

Reading this as: a glanceable companion to the main window, same dials (ENERGY 1 / RHYTHM 1 / MOTION 1).

| Element | Decision | Reason (R-31) |
|---|---|---|
| Status item | Monochrome template version of the Stacked stubs icon, then the running timer (`0:12`) when one runs, then the Needs you count | macOS menu bar items are monochrome; the timer and count are the two things worth a glance |
| Focal point | The running timer lives on its ticket's row: the row is tinted, its time shows the live `00:12:34` counter in bold, and its button becomes Stop. | No separate timer card: the time belongs to the ticket, and one control per ticket avoids two stop buttons |
| List | Grouped by iTop status, escalated first: Escalated TTR, Escalated TTO, Assigned, Pending. Each row has the ref, title, time and a start/stop icon button; a teal dot still marks tickets waiting on you. Clicking the title opens the ticket in the main window | The groups match the states you work with in iTop, so the panel reads like your iTop queue |
| Footer | Icon buttons: New ticket (⌘N), Refresh (⌘R), Open iTop Wrap (⌘O), with "Updated 1 min ago" between them | Quick actions without opening the window |
| Teal | Only the Needs you dots | Same single meaning as in the main window |
| Timer | The same timer as the main window: starting one in either place stops the other | One source of truth for time spent |

## People

- Same layout as Organizations: a searchable list on the left (initials avatar, name, "Agent · IT services" or "Caller · Org A", and an open-ticket count), details on the right.
- **Details:** avatar and name, then organization (links to the org view), email, phone and, for agents, their teams.
- **Tickets:** two tabs, "Assigned to them" and "Reported by them", each with a count. Agents open on the first tab, callers on the second. Rows are grouped by status, with the other person shown in the last column: the caller on the Assigned tab, the assignee on the Reported tab.

## Unassigned tickets

- **Look:** a dashed circle and the word "Unassigned", never just an empty cell.
- **Assign to me, one click:** unassigned rows in My tickets and in the Organizations table carry a dashed "Assign to me" pill in place of the assignee. In the ticket header, "Assign to me" replaces "Assign ticket" as the main action (teal, since it's waiting on you), and the More menu offers "Assign to someone else".
- **Assign sheet:** an "Assign to me" button sits at the top, then "or choose a team and agent" above the pickers.
- **After assigning:** the ticket moves to Assigned under your name, its main action becomes Resolve ticket, and the timer button appears.

## Search

- **What it matches:** the tickets in your list (main window and menu bar), by ref ("412" or "R-000412") or by words in the title. It narrows the sections as you type, and Escape clears it.
- **A ref outside your queue:** when you type a full ref that isn't in your list, the app looks it up in iTop and shows one result, "Open R-000512 · <title>", which opens the main window with that ticket selected. While the search is active, the list shows a single "Search result: not in your queue" section holding it, and the detail header adds "· Not in your queue". The ticket gets the normal detail view (logs, actions as far as your rights allow). It has no timer button, since it isn't assigned to you. Clearing the search brings back your four status sections. In the wireframe, switch on the searchDemo tweak on the main window or menu bar to see it. If iTop says you have no rights to it, or it doesn't exist, the result says so in plain words.
- **Escape hatch:** every ticket's More menu has "Open in iTop on the web" for anything the app doesn't cover (attachments, custom fields, history).
- **Not searched here:** people, CIs and closed tickets. The CMDB view covers people and CIs.

## Organizations

- **Organizations view** (sidebar, under CMDB): a list of organizations with their open-ticket count, and a detail pane for the selected one. The focal point is the org's open tickets from every agent, grouped by status like My tickets. The list has column headers (Ref, Title, Caller, Assignee). The Assignee column shows an initials avatar plus the name ("You", "Agent 2"), or a dashed circle and a bold "Unassigned". Rows also get a P1 mark. The teal dot still marks tickets waiting on you. Clicking a row opens it in the ticket detail view.
- **Secondary:** People and Config items links with their counts, and a New ticket button prefilled with the org.
- **Org filter:** a building-icon menu above My tickets and next to the menu bar's search. It's multi-select with "All organizations" to reset, and the button shows the chosen org(s) with a darker border when a filter is on. Empty sections hide while filtering. If none of your tickets match, it says so and links to the full org view.

## Date range and sorting (all ticket lists)

- **Range menu** (calendar icon) above My tickets, Team tickets, an organization's tickets and a person's tickets: "Open tickets" by default; "Opened in the last 7, 30 or 90 days"; and "Include resolved and closed". With resolved and closed included there's always a range (30 days by default, "Any time" is off), because iTop's OQL can't order results and an unbounded history can't be cut to the newest. Resolved and Closed get their own groups after the open ones. The label says what's shown: "Open tickets", "Open, last 30 days", "All, last 30 days".
- **Sort menu** (arrows icon): inside each status group, "Priority, then oldest" (default), "Last update, newest first", "Opened, newest first" or "Opened, oldest first". Groups never move.
- One setting for every list, remembered between launches. A chip's border goes dark when it's off the default, like the org filter.
- Org counts in the Organizations list stay open-ticket counts. "Needs you" only ever counts open tickets.

## Screens and focal points (one per screen)

| Screen | Focal point | Layout |
|---|---|---|
| Sign in | The "Sign in to iTop" button | Small centred window: server URL, username, password. An `http://` URL shows an inline warning |
| Ticket list | Tickets with the teal "needs you" dot, counted in the sidebar | Grouped by iTop status, the same as the menu bar, escalated first: Escalated TTR → Escalated TTO → Assigned → Pending. Row: needs-you dot, ref (motif), priority mark, age, title, caller, time spent |
| Ticket detail | Title plus the single primary action for the current state (Assign when new, Resolve when assigned, Reopen when resolved) | Header: ref · type · title · primary action, other actions in a More menu. Fields: caller, organisation, team/agent, linked CIs as links. Activity: Public/Private segmented control, entries oldest to newest, composer pinned at the bottom |
| CMDB | The search field | Search on top, results list (name, class, organisation, status), detail with main fields and an "Open tickets" section |
| Sheets (New ticket, Assign, Resolve, Set to pending) | The confirming button, named for its action | Short single-column forms; Escape cancels; ⌘↩ confirms |

## Required fields

- **Set to pending:** Reason is required. Resolve: Service, Service subcategory (prefilled from the ticket) and Solution are required. New ticket: Service and Service subcategory are required. Both labels say "Required" next to the field from the start, so the requirement is never only discovered as an error.
- Leaving the field empty and confirming shows an inline error under the field in danger red ("Enter why the ticket is waiting." / "Describe what fixed it. The caller sees this.") and sends nothing to iTop.
- Set to pending and Resolve both stop a running timer and save its time; the pending sheet says so ("Your timer stops and saves 0:12 to this ticket.").
- **The More menu only lists actions valid for the ticket's current state, minus the main action already shown as a button.** Assigned and Escalated TTR: Reassign, Set to pending. Pending: Reassign (no Set to pending). Escalated TTO: none (its main action is Assign). Copy ref is always there.

## Dropdowns (one component everywhere)

- **One dropdown component** is used for every picker in the app: Organization, Caller, Service, Service subcategory, Impact and Urgency (New ticket), Team and Agent (Assign), Service, Service subcategory and Resolution code (Resolve), Refresh interval and Start view (Settings). No plain system pop-up menus, so every list looks and behaves the same. In SwiftUI this is one custom `DropdownField` view, not a mix of `Picker` and a search field.
- **Look:** the same 32 px field with a chevron, and the same popover list below it (8 px radius, hairline border). A check mark marks the selected item and a grey highlight shows the keyboard position. The field border darkens while open. The org filter menus use the same check mark.
- **Search only where lists get long:** Organization and Caller show a magnifier and accept typing ("Search organizations", "Search people in Org A"). Short lists open the same popover without a search field.
- **Dependent lists:** Caller searches only the chosen organization, Subcategory only the chosen service, and Agent only the chosen team. Changing the parent clears the child. With no parent chosen, the child says so ("Choose an organization first").
- **One open at a time:** opening a dropdown closes any other. Clicking anywhere else, or Tab, Escape or picking an item, closes it.
- **Keyboard:** ↓/↑ opens the list and moves, ↩ picks, Esc closes, and typing filters and resets to the first match. Fields are real controls with `role="combobox"`, and the highlighted option is announced through `aria-activedescendant`. The focus ring wraps the whole field.
- **No match:** the list says so in words ("No caller in Org A matches").

## Copy

- Buttons name the action: "Sign in to iTop", "Create ticket", "Assign ticket", "Resolve ticket", "Add to public log", "Add to private log". Never "Submit" or "OK".
- No em dashes in UI text. Sentence case. No buzzwords.
- **Empty states:**
  - My tickets: "Nothing waiting on you. New assignments show up here."
  - Team tickets: "Your teams have no open tickets."
  - CMDB search: "Search by name, for example a server or application."
  - No ticket selected: "Select a ticket to see its details."
- **Loading:** a native progress indicator in the list area with "Loading tickets from iTop".
- **Errors:** say what happened, then what to do, and include iTop's own message when it gives one.
  - "Can't reach itop.example.com. Check your connection, then retry." (with a Retry button)
  - "iTop rejected the change: <iTop message>."
  - "Your password was rejected. Sign in again."

## Ticket detail: description

The detail pane shows the ticket's **Description** under the details row and above the log, in full and with line breaks kept. It's the caller's original report, so it sits before the conversation about it.

## Focus and links in both themes

- **Focus ring:** 2 px `#1D9E75` everywhere. It passes 3:1 against both the light (3.39:1) and dark (4.92:1) backgrounds, where the darker brand teal disappears in dark mode.
- **Links:** text links use the mode's teal text token (`#0F6E56` light, `#9FE1CB` dark), never a fixed colour.
- **Error lines:** under fields, errors sit in reserved space, so showing or clearing one never moves the buttons.

## Keyboard (R-32)

Full keyboard access with system focus rings. ⌘N new ticket · ⌘R refresh · ⌘F search · ⌘1/⌘2/⌘3/⌘4 sidebar sections · ↑/↓ move through the list · ↩ open · ⌘↩ send entry or confirm sheet · Esc close sheet · ⌘C on a selected row copies its ref · ⌘T start or stop the timer.

## App icon: 2. Stacked stubs (chosen 2026-11-10)

Three offset ticket stubs on a teal tile (`#0F6E56`): the back two in lighter teals (`#5DCAA5`, `#9FE1CB`), the front one white with a teal check mark. It says "work through your queue". The wireframe's Icons artboard shows it at 220, 64 and 32 px.

**Why this one (R-31):** it still reads at 32 px, where the "R-" text in the Ref stub concept becomes illegible, and it uses only the approved teal palette.

Ref stub and Tray are dropped. The icon doesn't use iTop's logo or name artwork. Before Task 10 the wireframe SVG gets redrawn as a production `AppIcon.icns` in all macOS sizes (16 to 1024 px), and you sign off on the final art.

## Team tickets

- Everything open in your teams, grouped like My tickets plus a **New** group for unassigned tickets that haven't escalated yet: Escalated TTR, Escalated TTO, New, Assigned, Pending.
- Each row shows the agent's initials; unassigned rows get the dashed "Assign to me".
- A team filter ("All my teams", or any of your teams) sits next to the organization filter. Teams come from iTop and can't be edited here.
- Teal follows the same rule: someone else's TTR is not teal. A one-line note under the filters says so.

## First run

After the first sign-in, one card at the top of My tickets asks to turn on notifications (Turn on / Not now) and mentions the menu bar. It never comes back after either answer; Settings has the same switch.

## Notifications

- Off until you turn them on. One per escalation, no sound, and Focus modes apply.
- Your TTR: "R-000367 needs you", "Escalated TTR: …", with Open ticket.
- An unassigned TTO in your team: the same, plus **Assign to me** right in the notification.
- Several at once (for example after the Mac wakes): grouped into one, "3 tickets need you", opening My tickets.

## Settings (⌘,)

Three tabs, macOS style: **General** (show in menu bar, open at login, how often to check iTop, which list opens first), **Notifications** (the one switch, the macOS-blocked case with Open System Settings, your teams read-only), **Account** (server, user, Keychain, REST check, Sign out with a confirmation that names the running timer). Switches are neutral (dark track when on), never teal.

## Config item detail

Header with class and status, the CI name in the mono motif, and actions: New ticket on this CI, copy name, open in iTop. Below: fields, **Relationships** (Depends on and Used by as clickable CI chips, plus a one-line impact sentence), then open tickets with status and agent.

## Delivery Gate (run at every checkpoint in PLAN.md)

The full antislop Delivery Gate runs at Checkpoints A to D against the running app, with a recorded click-through (R-35), checks in both light and dark mode (R-34), and keyboard-only passes (R-32). This document is a plan, so it can't pass R-35 yet. That happens at Checkpoint A.
