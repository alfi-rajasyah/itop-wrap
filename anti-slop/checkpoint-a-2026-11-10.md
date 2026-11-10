# Checkpoint A: Delivery Gate on the running app (sign in and shell)

antislop mode: during. Direction: DESIGN.md. Build: `task-2-sign-in`, debug.
Checked with a made-up account only (`ITOPWRAP_DEMO`, `ITOPWRAP_SETTINGS`); nothing touched the real iTop.

## Findings and fixes

| # | Priority | Rule | Finding | Status |
|---|---|---|---|---|
| 1 | HIGH | C-4 / R-27 | Shell content laid out taller than the window: the first sidebar rows and the page heading sat under the toolbar, and the account button was cut off at the bottom. Cause: a minimum-size `.frame` on the `NavigationSplitView` itself. | Fixed: the minimum size moved to the detail pane. Seen correct in light and dark. |
| 2 | MEDIUM | C-3 | "My tickets" shown twice: window title and a page heading. The wireframes put it in the toolbar only. | Fixed: the page heading was removed. |
| 3 | MEDIUM | C-4 | With `.contentMinSize` plus a default size, sign-in opened as a 1100×720 window with the form floating in the middle. | Fixed: back to `.contentSize` (now safe after #1). Sign-in opens at 480×520. |

## Checked

- **Sign in, light and dark:** matches SignIn.dc.html. The button is neutral (near-black in light, near-white in dark), never teal.
- **Shell, light and dark:** the sidebar has Tickets (My tickets, Team tickets) and CMDB (Config items, Organizations, People), the title is in the toolbar, and the account button sits at the bottom.
- **Validation:** Return with empty fields shows "Enter your username and password." (driven in the running app).
- **R-02:** no em dashes in UI strings.
- **R-25:** colours come from the DESIGN.md tokens checked earlier (secondary text 5.3:1 or more, warning red on white and on dark).
- **R-26:** each sidebar section opens a placeholder that says what it is and what comes next. No control does nothing.
- **Real server:** the user signed in against their iTop and confirmed the details shown.

## Decision recorded

The sidebar selection uses the user's macOS accent colour (blue by default). That's the system's own control and the user's setting, so the app doesn't override it. Teal stays reserved for "needs you".

## Not covered automatically

Background control couldn't move keyboard focus between SwiftUI text fields, so the rest of the keyboard pass is a short manual check:
- Tab moves Server URL → Username → Password → Sign in, and Shift-Tab goes back
- ⌘1 to ⌘5 switch sidebar sections
- Esc closes the sign-out confirmation

## Delivery Gate status: PASS (keyboard check confirmed by the user, 2026-11-10)
