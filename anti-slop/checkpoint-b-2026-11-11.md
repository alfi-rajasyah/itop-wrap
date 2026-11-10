# Checkpoint B: Delivery Gate on the read-only app

antislop mode: during. Direction: DESIGN.md. Build: `checkpoint-b`, debug, demo data only
(`ITOPWRAP_DEMO`, `ITOPWRAP_SETTINGS`, `ITOPWRAP_DEMO_STATE`); nothing touched the real iTop.

Covered: My tickets, Team tickets, ticket detail (description, images, links, attachments, both logs),
Config items, Organizations, People; light and dark; empty, loading and error states; keyboard.

## Findings and fixes

| # | Priority | Rule | Finding | Status |
|---|---|---|---|---|
| 1 | MEDIUM | C-3 | The View button sat at the far right, away from the org filter: the filter's width cap made it always 220 points wide. | Fixed: the filter is as wide as its name, up to 200 points. |
| 2 | LOW | C-1 | Config item header: "Copy name" was a bordered button, "Open in iTop on the web" a bare icon. | Fixed: both bordered icon buttons (built; not re-checked on screen). |
| 3 | HIGH | C-4 / R-27 | An empty list hid the org filter and View menu, so with no open tickets you couldn't switch to resolved and closed ones. | Fixed: the controls stay above an empty list; an empty range says "No tickets in this range. Try a longer range in the View menu." |
| 4 | MEDIUM | Copy | The network error didn't match DESIGN.md's copy. | Fixed: "Can't reach itop.example.com. Check your connection, then retry." with Retry, in every view. |

## Checked

- **Light and dark:** list, detail with inline image, link, outside-image chip, signature icons and attachments; Config items, Organizations, People. Teal only marks "needs you"; filter buttons are neutral.
- **States:** loading ("Loading tickets from iTop"), empty ("Nothing waiting on you. New assignments show up here."), error (with Retry); People and Config items empty search hints; "Select a ticket to see its details."
- **Keyboard:** ↓/↑ move through the ticket list and open the detail; View menu items "My tickets" to "People" (⌘1 to ⌘5) switch sections; ⌘R refreshes; Esc closes the image viewer.
- **R-02:** no em dashes in UI strings. **R-26:** no button or link without an action; no placeholder text left.
- **R-25:** colours are the DESIGN.md tokens checked earlier; no new colour pairs.
- **NSTableView reentrancy:** selections are deferred and the list controls sit outside the lists; your traced run on real data logged none.

## Real data (SPEC user stories 2, 3, 6, 9, 10)

Confirmed by you while building Tasks 3 to 5c: My tickets and Team tickets match your queue (2), ticket detail and logs match the web UI (3), CMDB search, open tickets and relationships match (6), Organizations with the range menu match (9), People search and tabs work (10).

## Delivery Gate status: PASS

Waiting only on your review before any write feature is built.
