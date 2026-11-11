# Checkpoint C: Delivery Gate on the write features

antislop mode: during. Direction: DESIGN.md (dials ENERGY 1 / RHYTHM 1 / MOTION 1). Build: `checkpoint-c`, debug,
demo data (`ITOPWRAP_DEMO`, `ITOPWRAP_APPEARANCE=light|dark`); nothing touched the real iTop during the gate.

Covered: log composer, Assign to me (row pill, header, sheet), Assign sheet, Set to pending, Resolve, Reopen, Close
(confirmation), the timer (header, rows, detail strip, notices, Retry), the menu bar item and panel, New ticket, and the
shared dropdown in every sheet; light and dark; keyboard.

## Findings and fixes

| # | Priority | Rule | Finding | Status |
|---|---|---|---|---|
| 1 | HIGH | R-25 | The teal main action (Assign to me, Resolve ticket when the ticket needs you) had white text on the dark-mode teal: 2.0:1. The system's prominent button forces white text on any tint. | Fixed: the header's main action has its own style; near-black on the dark teal (8.4:1), white on the light teal (6.2:1). Checked on screen in both themes. |
| 2 | MEDIUM | C-1 / R-31 | Header timer was a "Start timer" text button and "Stop 0:00:00"; DESIGN.md asks for an icon-only stopwatch, then a stop icon with a live `00:12:34`. | Fixed and checked in both themes. |
| 3 | MEDIUM | C-1 / R-31 | Row times read `1:15` in the system font with a timer icon; DESIGN.md asks for `1h 20m` totals and a stopwatch with `0:12` in bold, in the refs' SF Mono. Same in the menu bar panel and the detail strip. | Fixed (tested formats: `TimeFormat.total`, `TimeFormat.counter`) and checked on screen. |
| 4 | MEDIUM | Copy | Required-field errors read "Enter a reason." / "Enter a solution."; DESIGN.md has "Enter why the ticket is waiting." / "Describe what fixed it. The caller sees this." The pending and resolve sheets didn't say the timer stops and saves. | Fixed; "Your timer stops and saves 0:12 to this ticket." shows while it runs. Checked in Resolve. |
| 5 | LOW | Icons | Time spent in the detail strip used the timer icon, not the stopwatch. | Fixed. |
| 6 | LOW | Icons | New ticket had a help line under its title ("Organization first, then the caller"); DESIGN.md keeps hints out of sheets. | Removed; the subtitle only names the CI when there is one. |
| 7 | LOW | Icons | The composer's button had no send icon. | Added (built; not re-checked on screen). |

Found and fixed earlier in this phase, before the gate: the dropdown didn't follow DESIGN.md (rebuilt, see PLAN Task 9),
the ticket header squeezed in narrow windows (now stacks), the menu bar panel's list collapsed to nothing, two notices
drew on top of each other, and app time showed teal in iTop's calendar (now origin `stopwatch`).

## Checked

- **Light and dark:** header with teal and neutral main actions, timer idle and running, rows with totals and the running
  marker, Resolve with prefill, required errors and the timer note; New ticket, Assign and dropdown lists in dark.
- **Keyboard (R-32):** dropdowns by ↓/↑/↩/Esc and typing; Tab through sheets; ⌘↩ confirms; Esc closes an open list
  before it cancels the sheet; ⌘T timer; ⌘N New ticket; ⌘O reopens the window.
- **States (R-27):** sheet lists show loading and "Couldn't load: …"; empty required fields block with inline errors;
  iTop refusals keep the sheet or draft open with iTop's message; a failed time save keeps the time with Retry.
- **R-26:** no control without an action; actions iTop wouldn't allow for the state aren't offered; disabled controls
  say why ("switched off in this build"). New ticket in the menu bar arrived with Task 9.
- **R-02:** no em dashes in UI strings (the only one is the HTML entity table). **R-15/R-16:** buttons name their action.
- **R-23:** the menu bar icon was drawn only after you chose it; you reviewed the preview.

## Real data (SPEC user stories 4, 5, 7, 8)

| Write | Real check |
|---|---|
| Add a log entry (Task 6) | Done 2026-11-11 on an allowlisted test ticket; shows in the web UI history with your name. |
| Time spent (Task 8b) | Done 2026-11-11; entries show under your name, grey like the web stopwatch after the origin fix. |
| Menu bar (Task 8c) | Checked on your real iTop, read-only; the count matched. |
| Reassign (Task 7), pending, resolve, reopen, close (Task 8), New ticket (Task 9) | **Open.** Planned as one run on a new test ticket with you as caller (`ITOPWRAP_WRITE_NEW=1`); you chose not now. |

Test data: the log entries and time entries on your test ticket from earlier checks; you'll delete the ones you don't want in the web UI.

## Delivery Gate status: PASS for the UI; real checks for stories 4 and 5 still open
