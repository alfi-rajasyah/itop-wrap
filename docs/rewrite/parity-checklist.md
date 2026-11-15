# Parity checklist: SwiftUI iTop Wrap → Tauri (Rust + React)

The Tauri rewrite is checked against this list. Each item is a concrete, testable behaviour of the
current app (v1.0.2, commit 576e7b4), with the source it comes from. **The code is the authority,
not SPEC.md / UX-FLOWS.md / DESIGN.md**: several things those documents describe were never built
(see section 21). Rebuild what the code does, nothing more.

Paths are relative to the repo root. `W/` = `Sources/ITopWrap/`, `K/` = `Sources/ITopKit/`.
Strings in quotes are exact user-facing text (curly quotes “ ” and ellipses … are literal
characters; `<ref>`, `<name>` etc. are placeholders). "Tooltip" means the `.help` text, which is also
the hover text; "a11y" means the accessibility label.

---

## 1. Sign in & session

### Launch and restore
- AUTH-1: On launch, while no session exists, the main window shows a small spinner with "Signing in…" in a 480×320 window, and the app tries to restore the saved session. (W/App.swift:19, W/App.swift:145-148)
- AUTH-2: Restore uses the saved server and username (UserDefaults `server`, `user`) and the Keychain password for account `"<user>@<baseURL>"`. If any of the three is missing, or the saved server no longer parses, it shows the sign-in form with no notice. (K/SessionService.swift:82-89, W/SessionModel.swift:67-73)
- AUTH-3: If restore fails with invalid credentials, the sign-in form shows the notice "Your password was rejected. Sign in again." (W/SessionModel.swift:74-75)
- AUTH-4: Any **other** restore error (network, HTTP error, REST-profile refusal, certificate, Keychain read error, non-iTop answer) shows the "Can't reach iTop" screen with that error's own description, not the sign-in form. (W/SessionModel.swift:76-77)
- AUTH-5: The "Can't reach iTop" screen (480×320): title "Can't reach iTop", the error text, a filled "Try again" button (Return triggers it) that re-runs restore, and a link-style "Sign in with another account" that goes to the sign-in form **without** deleting the Keychain password. (W/ShellView.swift:190-211, W/SessionModel.swift:114-116)
- AUTH-6: Opening a session = `core/check_credentials` must answer `authorized: true` (else invalid credentials), then the current user is loaded. (K/SessionService.swift:96-99, K/ITopClient.swift:133-136)
- AUTH-7: Loading the current user: `SELECT User WHERE login = '<login>'` with fields login, contactid, first_name, last_name, profile_list; no User found → invalid credentials. Then, if contactid ≠ 0, `SELECT Person WHERE id = <contactid>` with name, first_name, team_list. Teams come from `team_list` (`team_id`, `team_name`, fallback name "Team <id>"). Display name = Person "first_name name", else User "first_name last_name", else the login. (K/CurrentUser.swift:41-71)
- AUTH-8: Initials for the account button: first letters of the first and last word of the display name (words starting with a letter), uppercased ("Jane van Doe" → "JD"); one word → one letter; none → first two characters of the login, uppercased. (K/CurrentUser.swift:33-38)

### Sign-in form
- AUTH-9: Window 480×520. Heading "Sign in to iTop" (20 pt semibold), subtitle "Use your iTop account. Your password stays in the macOS Keychain." (W/LoginView.swift:21-26, W/LoginView.swift:70)
- AUTH-10: Fields, each with a label above: "Server URL" (placeholder "https://itop.example.com"), "Username" (no placeholder), "Password" (secure field). (W/LoginView.swift:30-51)
- AUTH-11: The server field starts with the saved server, or "https://" when none is saved; username starts with the saved username or empty. Password always starts empty. (W/SessionModel.swift:55-59)
- AUTH-12: Initial focus: Server URL if its text is ≤ 8 characters (i.e. just "https://"), else Username if empty, else Password. (W/LoginView.swift:72-74)
- AUTH-13: When the server text (trimmed, lowercased) starts with `http://`, a warning shows under the field: "This address isn't encrypted (http). Your password would be sent in plain text. Use https if your server supports it." Signing in over http is still allowed. (W/LoginView.swift:35-41, W/LoginView.swift:77-79)
- AUTH-14: Submit by the button, Return (default action) or submitting any field. A second submit while signing in is ignored. (W/LoginView.swift:55-63, W/LoginView.swift:71, W/LoginView.swift:108-109)
- AUTH-15: If the username (trimmed of spaces) or the password is empty: message "Enter your username and password." and focus moves to the empty username, else to the password. Nothing is sent. (W/LoginView.swift:111-115, W/LoginView.swift:83-85)
- AUTH-16: While signing in, the button shows a small spinner and reads "Signing in…" and is disabled. The fields are **not** locked. (W/LoginView.swift:56-63)
- AUTH-17: REST-profile refusal shows a bordered box: bold warning-coloured "This account can't use the iTop REST API." followed by " Ask your iTop admin to add the "REST Services User" profile to <username>, then sign in again." (W/LoginView.swift:86-91)
- AUTH-18: Any other ITopError shows its description (section 18) in warning colour; a non-ITopError shows the "unexpected response" text. Invalid credentials also moves focus to Password. (W/LoginView.swift:92-95, W/LoginView.swift:122-127)
- AUTH-19: The restore notice (AUTH-3) only shows when there's no missing-fields message and no error. Messages show in this priority: missing fields, REST box, error, notice. (W/LoginView.swift:82-99)
- AUTH-20: On success: the password is saved to the Keychain only after credentials and user load both succeed; then the server is saved as the normalised base URL and the trimmed username; the form's server field is rewritten to the normalised URL; the sidebar selection resets to My tickets; the password field is cleared. Nothing is saved if any step fails. (K/SessionService.swift:70-78, W/SessionModel.swift:82-87, W/LoginView.swift:121)
- AUTH-21: The username is trimmed of whitespace and newlines before use; the password is sent exactly as typed. (K/SessionService.swift:72)

### Server address normalisation
- AUTH-22: Input is trimmed; empty → "That server address can't be used. Enter it like https://itop.example.com." (K/ServerAddress.swift:20-21, K/ITopError.swift:30-31)
- AUTH-23: No `://` → `https://` is prefixed (a bare host means https). (K/ServerAddress.swift:22)
- AUTH-24: Only http/https with a non-empty host are accepted. (K/ServerAddress.swift:24-28)
- AUTH-25: A pasted iTop page or REST URL is cut back at `/pages/` or `/webservices/`; trailing slashes removed; port kept; query and fragment dropped; scheme lowercased. (K/ServerAddress.swift:30-40)
- AUTH-26: REST endpoint = `<base>/webservices/rest.php?version=1.4`. (K/ServerAddress.swift:8, K/ServerAddress.swift:15-17)

### Sign out
- AUTH-27: The account button is a 28 pt circle with the user's initials at the bottom of the sidebar, above a divider. Tooltip "<displayName> · <host>"; a11y "Account: signed in as <displayName> on <host>". (W/ShellView.swift:162-186)
- AUTH-28: Its menu shows a disabled line "<displayName> · <host>", a divider, and "Sign out…". (W/ShellView.swift:164-168)
- AUTH-29: "Sign out…" asks with a confirmation dialog titled "Sign out of <host>?", message "Your password is removed from the Keychain. Your username stays filled in.", destructive button "Sign out" (plus the system Cancel). `<host>` is the server's host, or "iTop" if none. (W/ShellView.swift:95-99, W/ShellView.swift:143)
- AUTH-30: Signing out deletes the Keychain item (a missing item is not an error), keeps server and username, and shows the sign-in form with no notice. A Keychain delete error is swallowed. (W/SessionModel.swift:108-111, K/SessionService.swift:92-94, K/KeychainStore.swift:52-55)
- AUTH-31: Signing out does **not** stop or clear a running timer: it stays saved and comes back at the next sign-in. (W/SessionModel.swift:101-104, W/TimerModel.swift:49)
- AUTH-32: Signing in again as the same user on the same server keeps the existing workspace (lists, drafts, timer); a different user or server builds a new one. (W/SessionModel.swift:94-99)
- AUTH-33: There is no mid-session re-login: if the password stops working while signed in, calls simply fail with "Invalid credentials. Check your username and password." where the error shows. (K/ITopClient.swift:124-127; no session-expired UI exists)

### Every REST call
- AUTH-34: Each call is an HTTP POST, `Content-Type: application/x-www-form-urlencoded`, with fields `auth_user`, `auth_pwd`, `json_data`, in that order. Every value is percent-encoded except RFC 3986 unreserved characters (A–Z a–z 0–9 - . _ ~), so `&`, `=`, `+` in a password survive. (K/ITopClient.swift:91-98, K/ITopClient.swift:147-154)
- AUTH-35: `json_data` is JSON with sorted keys and unescaped slashes; whole numbers are encoded as integers. (K/ITopClient.swift:140-144, K/JSONValue.swift:26-27)
- AUTH-36: Writes carry `comment: "iTop Wrap"`; create/update/stimulus send `output_fields: "id"` (InlineImage create asks for `id,secret`). (K/TicketWrites.swift:13, K/RESTOperation.swift:56-73, K/LogImage.swift:100)
- AUTH-37: TLS validation is never disabled; the app trusts only what macOS trusts. (K/ITopClient.swift:192-200; README Troubleshooting)
- AUTH-38: `core/delete` is never used anywhere. (K/RESTOperation.swift:4-15)

---

## 2. App shell & sidebar

- SHELL-1: One main window titled "iTop Wrap" (a single-instance window, not a window group). Its size follows its content: the fixed sign-in/restore/unreachable sizes, then the resizable shell. (W/App.swift:16-22)
- SHELL-2: The shell is a three-column split view: sidebar (min 180, ideal 200, max 260 pt), content (min 300, ideal 360, max 460 pt), detail (min 380 × 520 pt). (W/ShellView.swift:32, W/ShellView.swift:45, W/ShellView.swift:87)
- SHELL-3: Sidebar section "Tickets": "My tickets" (SF Symbol `tray`), "Team tickets" (`person.2`). Section "CMDB": "Config items" (`server.rack`), "Organizations" (`building.2`), "People" (`person.text.rectangle`). (W/ShellView.swift:22-31, W/SessionModel.swift:5-28)
- SHELL-4: "My tickets" shows a teal capsule with the Needs-you count when it's > 0 (11 pt semibold, tabular digits); a11y and tooltip "<n> need you". Team tickets has no count. (W/ShellView.swift:23, W/ShellView.swift:145-160)
- SHELL-5: The content column's title is the selected section's title. (W/ShellView.swift:44)
- SHELL-6: The selected section defaults to My tickets at launch and after sign-in; it isn't remembered across launches. (W/SessionModel.swift:49, W/SessionModel.swift:85)
- SHELL-7: Detail placeholders when nothing is selected: "Select a ticket to see its details." (My/Team), "Select a config item to see its details.", "Select an organization to see its open tickets.", "Select a person to see their details and tickets." (W/ShellView.swift:52, W/ShellView.swift:61, W/ShellView.swift:73, W/ShellView.swift:79)
- SHELL-8: Selection clicks in the sidebar and in every list are applied one run-loop turn after the click (avoids re-entrancy). A rewrite needs no hack but must give the same end state. (W/Theme.swift:117-130)
- SHELL-9: Opening a ticket from a CI, an org or a person (`openTicket`): if My tickets has it, clear My tickets' search, select it, switch to My tickets; else if Team tickets has it, same in Team tickets; else put its ref into My tickets' search and switch to My tickets **without selecting it**: the full-ref lookup then offers it as "Search result: not in your queue" and you click it. (W/ShellView.swift:120-133)
- SHELL-10: Opening an organization (from "See all of its tickets" or a person's organization): clear the org search, select that org id, switch to Organizations. (W/ShellView.swift:107-111)
- SHELL-11: A ticket's linked CI opens in Config items as the current CI (class and name from the link, org empty), even though it isn't in the search results. (W/ShellView.swift:114-117, W/CMDBModel.swift:101-104)
- SHELL-12: From an org: "People" switches to People scoped to that org; "Config items" switches to Config items scoped to that org; "New ticket" opens New ticket prefilled with that org (id and name). (W/ShellView.swift:67-70)
- SHELL-13: From a CI: "New ticket" opens New ticket with that CI linked and the org chosen by the CI's org **name**. (W/ShellView.swift:58-59)
- SHELL-14: New ticket requested in the main window opens as a sheet over the shell. (W/ShellView.swift:89-94)
- SHELL-15: The app is a regular app with a Dock icon and comes to the front at launch. (W/App.swift:161-166)
- SHELL-16: Signed in, closing the main window does **not** quit (the menu bar item keeps working). Signed out (or restoring/unreachable), closing the last window quits the app. (W/App.swift:170-172, W/SessionModel.swift:100-104)

---

## 3. Ticket lists (My tickets / Team tickets)

### What is loaded
- LIST-1: Both lists query both classes, `UserRequest` and `Incident`, concurrently, and merge them. (K/TicketQueries.swift:92-116)
- LIST-2: My tickets: `SELECT <cls> WHERE <scope> AND (agent_id = <myContactID> OR (status = 'escalated_tto' AND agent_id = 0 AND team_id IN (<myTeamIDs>)))`; with no teams, just `... AND agent_id = <me>`. Team ids that aren't integers are dropped. (K/TicketQueries.swift:59-63, K/TicketQueries.swift:79-81)
- LIST-3: Team tickets: `SELECT <cls> WHERE <scope> AND team_id IN (<myTeamIDs>)`; no teams → no query at all. (K/TicketQueries.swift:66-69)
- LIST-4: Scope clause: default (open) = `status NOT IN ('resolved','closed')`; with a range, `AND start_date >= DATE_SUB(NOW(), INTERVAL <days> DAY)`; including resolved and closed with any time = `id > 0`. The date test runs on the server. (K/TicketQueries.swift:19-25)
- LIST-5: Output fields per row: ref, title, status, priority, caller_id_friendlyname, org_id, org_id_friendlyname, team_id, team_id_friendlyname, agent_id, agent_id_friendlyname, start_date, last_update. (K/Ticket.swift:54-56)
- LIST-6: Parsing: ids are strings, "0" when empty; a missing priority counts as 4; numbers may arrive as JSON strings or numbers. Dates are `yyyy-MM-dd HH:mm:ss` (POSIX) read in the Mac's local time zone. (K/Ticket.swift:58-84, K/JSONValue.swift:47-53)
- LIST-7: A ticket is unassigned when agent_id is "0" or empty. Ticket identity is `"<Class>::<key>"`. (K/Ticket.swift:14, K/Ticket.swift:51)
- LIST-8: iTop returns objects as a keyed map; they are ordered by numeric id before use. (K/ITopClient.swift:218-222)

### Needs you
- LIST-9: A ticket needs you only when it's `escalated_ttr` and assigned to you, or `escalated_tto`, unassigned, with its team among your teams. Nothing else. (K/TicketQueries.swift:159-165)
- LIST-10: The sidebar count and the menu bar count are the number of My tickets (all loaded tickets, ignoring the org filter and search) that need you. Resolved/closed never qualify. (W/TicketListModel.swift:108)

### Grouping and sorting
- LIST-11: Status → group: escalated_ttr "Escalated TTR", escalated_tto "Escalated TTO", new "New", assigned "Assigned", pending "Pending", resolved "Resolved", closed "Closed", anything else "Other". (K/TicketQueries.swift:119-147)
- LIST-12: Group order, My tickets: Escalated TTR, Escalated TTO, Assigned, New, Pending, Other, Resolved, Closed. Team tickets (and the org/person tables): Escalated TTR, Escalated TTO, New, Assigned, Pending, Other, Resolved, Closed. Empty groups are left out. (K/TicketQueries.swift:169-178)
- LIST-13: Group header: the group title on the left and the group's **whole** count on the right (not just the page's share), 11 pt semibold secondary. (W/TicketListView.swift:95-103, K/TicketQueries.swift:220)
- LIST-14: Sort inside each group (groups never move): "Priority, then oldest" (default) = priority ascending, then start date ascending (no date last), then ref ascending; "Last update, newest first" = last_update descending (no date counts as oldest), tie by ref ascending; "Opened, newest first" = start date descending, tie ref ascending; "Opened, oldest first" = start date ascending (no date last), tie ref ascending. (K/TicketQueries.swift:29-52)

### Search (local)
- LIST-15: Search field in the toolbar, prompt "R-000412 or VPN". (W/TicketListView.swift:18)
- LIST-16: The query is lowercased and split on spaces; every word must appear in `ref + " " + title` (lowercased). Order is kept. Matching covers the list's loaded tickets only. (K/TicketQueries.swift:238-245)
- LIST-17: The org filter is applied first, then the search, then grouping, then paging. (W/TicketListModel.swift:110-114)
- LIST-18: While searching with no matches, no lookup result and no lookup pending: "No tickets in your list match “<trimmed query>”." (W/TicketListView.swift:83-87)
- LIST-19: Searching never shows the empty-list message; the empty message (LIST-44) only applies when not searching. (W/TicketListView.swift:61)

### Full-ref lookup
- LIST-20: A lookup runs only when the trimmed, uppercased query is exactly `R-` or `I-` followed by 6 digits, and no loaded ticket has that ref (case-insensitive). A partial ref ("412", "R-0004") only filters locally. Digits alone are never looked up. (W/TicketListModel.swift:182-188, K/TicketQueries.swift:72-77)
- LIST-21: The lookup query is `SELECT UserRequest WHERE ref = 'R-000512'` (R → UserRequest, I → Incident), string quoted with `\` and `'` escaped. (K/TicketQueries.swift:76, K/OQL.swift:5-7)
- LIST-22: The state goes to "searching" at once, then waits 250 ms; a new keystroke cancels the wait and the call. The answer is dropped if the query changed meanwhile. (W/TicketListModel.swift:191-197, W/TicketListView.swift:19)
- LIST-23: If the same ref was already found, it isn't looked up again. (W/TicketListModel.swift:190)
- LIST-24: Searching shows a bar above the list: magnifier icon + "Looking up <REF> in iTop…". (W/TicketListView.swift:195-196)
- LIST-25: Not found (or no rights): bar "<REF> doesn't exist, or your iTop account can't see it." (W/TicketListView.swift:197-198)
- LIST-26: Lookup error: bar in warning colour "Couldn't look that up: <error>". (W/TicketListView.swift:199-200)
- LIST-27: Found: one extra section at the top titled "Search result: not in your queue" with that ticket's row (agent always shown; no time total, no running timer shown on that row). Selecting it opens the normal detail with all actions as far as iTop allows. (W/TicketListView.swift:173-179)
- LIST-28: Any query that isn't a full ref clears the lookup. (W/TicketListModel.swift:184-188)
- LIST-29: After every list reload, a found ticket is dropped if the list now contains it (the list's copy wins), otherwise it is read again so it shows its current state. (W/TicketListModel.swift:170-179; Tests/ITopWrapTests/LookupRefreshTests.swift:58-81)

### Resolved/closed "just now"
- LIST-30: Resolving or closing the **selected** ticket keeps it selected and on screen even when the view hides resolved and closed tickets: it becomes the extra section, titled "<Status> just now" (e.g. "Resolved just now", "Closed just now"), showing its new status. (W/TicketListModel.swift:289-292, W/TicketListView.swift:175; Tests/ITopWrapTests/ResolveKeepsOpenTests.swift:65-88)
- LIST-31: You can close it from there; it then shows as closed, still selected. (Tests/ITopWrapTests/ResolveKeepsOpenTests.swift:78-88)
- LIST-32: It disappears at the first reload after you select another ticket, or as soon as the lookup changes for any other reason. Resolving a ticket that isn't selected changes nothing about the selection. (W/TicketListModel.swift:79-81, W/TicketListModel.swift:172-173; Tests/ITopWrapTests/ResolveKeepsOpenTests.swift:90-100)

### Org filter (My tickets only)
- LIST-33: The org filter menu shows only on My tickets, only when the loaded tickets span more than one org name or a filter is already on. Team tickets never has it. (W/TicketListView.swift:116, W/ShellView.swift:37-38)
- LIST-34: Choices are the distinct non-empty org names of the loaded tickets, sorted with natural (Finder-style) comparison. (K/TicketQueries.swift:233-235)
- LIST-35: Menu: "All organizations" (check mark when no filter), divider, each org (check mark on the chosen one). Picking the chosen org again turns the filter off. Single select. (W/TicketListView.swift:134-143, W/TicketListModel.swift:123-126)
- LIST-36: Button: building icon + the chosen org name or "All organizations", truncated, max 200 pt; border dark when a filter is on, grey when off. Tooltip "Showing tickets for <org>" or "Filter by organization"; a11y "Filter by organization: <org or all>". (W/TicketListView.swift:144-154, W/ListOptions.swift:138-153)
- LIST-37: With a filter on, nothing matching and not searching: "None of your tickets are for <org>." plus a link "See all of its tickets" (only when some loaded ticket gives that org's id, i.e. org_id ≠ 0), which opens the org in Organizations. (W/TicketListView.swift:72-74, W/TicketListView.swift:158-169, W/TicketListModel.swift:119-121)
- LIST-38: The org filter is not remembered across launches and does not affect the Needs-you count or the menu bar panel. (W/TicketListModel.swift:88, W/TicketListModel.swift:108, W/MenuBarView.swift:64-68)

### View menu (date range, resolved/closed, sort, page size)
- LIST-39: One "View" button (icon `line.3.horizontal.decrease`) above My tickets, Team tickets, an org's tickets and a person's tickets; all four share one setting. Border dark when the view isn't the default. (W/ListOptions.swift:78-119, W/OrgsView.swift:192, W/OrgsView.swift:332)
- LIST-40: Menu sections: "Show tickets opened" with radio items "Any time", "Last 7 days", "Last 30 days", "Last 90 days" and a toggle "Include resolved and closed"; "Sort inside each status group" with the four sorts (LIST-14 titles); "Per page" with "10 per page", "20 per page", "30 per page", "50 per page", "100 per page", "All". When not default: a divider and "Back to default view". (W/ListOptions.swift:83-108, W/ListOptions.swift:9-21, W/ListOptions.swift:30)
- LIST-41: Default view = Any time, resolved/closed excluded, "Priority, then oldest", 20 per page. "Back to default view" restores exactly that. (W/ListOptions.swift:45-54)
- LIST-42: Summary of what differs, joined by " · ": the range label (only when the scope isn't plain open), the sort title (when not Priority), and "<n> per page" or "All on one page" (when not 20). Range labels: "Open tickets"; "Open, last 7 days" / "Open, last 30 days" / "Open, last 90 days"; "All tickets, any time"; "All, last 7 days" etc. (W/ListOptions.swift:57-73)
- LIST-43: The summary shows as a quiet 11 pt line under the controls (up to 2 lines) only when non-default. The View button tooltip is the summary, or "Open tickets, sorted by priority, 20 per page" by default; a11y "View options: <summary or default>". (W/ListOptions.swift:116-117, W/ListOptions.swift:122-134)
- LIST-44: Empty list (loaded, not searching): if the scope isn't plain open, "No tickets in this range. Try a longer range in the View menu."; else My tickets "Nothing waiting on you. New assignments show up here.", Team tickets "Your teams have no open tickets." (W/TicketListView.swift:61-63)
- LIST-45: The org filter and View controls sit above the list (not scrolling) whenever the list is loaded, even when it's empty, so you can widen the range; they're hidden while loading, on a failed first load, and on Team tickets with no teams. (W/TicketListView.swift:14-16, W/TicketListView.swift:113-124)
- LIST-46: Changing the range or the resolved/closed toggle reloads the list from iTop and goes back to page 1. Changing sort, page size, search or org filter only goes back to page 1 (no reload). (W/TicketListView.swift:20-24)
- LIST-47: Resolved and Closed tickets get their own groups, after the open ones. (K/TicketQueries.swift:171-172)

### Paging
- LIST-48: Paging happens in the app after grouping and sorting: page 1 holds the most urgent tickets; a page can cut a group in two. "All" means one page. (K/TicketQueries.swift:183-218)
- LIST-49: Pager under the list only when there's more than one page: "‹" button, "<first>–<last> of <total>" (en dash, tabular digits), "›" button; ‹ disabled on page 1, › on the last page; tooltips/a11y "Previous page", "Next page". With one page and ≥ 1 ticket it shows "1 ticket" or "<n> tickets"; with none, nothing. (W/ListOptions.swift:156-178)
- LIST-50: A requested page past the end is clamped to the last page. (K/TicketQueries.swift:200-201)
- LIST-51: The page is per list (My and Team each have their own) and isn't remembered. (W/TicketListModel.swift:97)

### Rows
- LIST-52: Row line 1: teal 8 pt dot when it needs you (a11y/tooltip "Needs you"), the ref in monospace 11.5 pt secondary, the priority mark, then right-aligned the age. (W/TicketListView.swift:297-313)
- LIST-53: Priority mark: P1 a bold red "!" (a11y/tooltip "Priority 1"); P2 a 6 pt grey dot ("Priority 2"); P3/P4 nothing. (W/TicketListView.swift:343-355)
- LIST-54: Age from start date: under 60 s "now", under an hour "<n>m", under a day "<n>h", else "<n>d" (floor). Tooltip "Opened <date, abbreviated> <time, short>". No start date → no age. (K/TicketQueries.swift:250-258, W/TicketListView.swift:308-312)
- LIST-55: Line 2: the title, 13 pt medium, up to 2 lines. (W/TicketListView.swift:314-316)
- LIST-56: Line 3: caller name (1 line), then right-aligned: the running timer (stopwatch icon + live `h:mm`, bold monospace, a11y/tooltip "Timer running"), or else the total time when > 0 ("1h 20m", tooltip "Time spent", a11y "<total> spent"), then the agent when shown. (W/TicketListView.swift:317-334)
- LIST-57: The agent shows on every Team tickets row and on unassigned My tickets rows: a dashed "Assign to me" pill when the ticket is unassigned, can be assigned (its state has assign or reassign) and writes are allowed for it; else a dashed circle + bold "Unassigned"; else "You" for your own; else the agent's name. (W/TicketListView.swift:91, W/TicketListView.swift:357-381, W/TicketListView.swift:182-188)
- LIST-58: The pill's tooltip "Assign <ref> to you", a11y "Assign <ref> to me". Clicking it selects the ticket and assigns it to you (ASG-1). (W/TicketListView.swift:183-187, W/TicketListView.swift:368-369)
- LIST-59: Time format helpers: `clock` "h:mm" (e.g. "0:42", "10:05"); `total` "45m", "2h", "1h 20m" (minutes floored, 0 → "0m"); `counter` "HH:MM:SS" zero-padded. (K/TimeTracking.swift:59-78)

### States, refresh and footer
- LIST-60: First load: small spinner + "Loading tickets from iTop" in the list area. (W/TicketListView.swift:50-55)
- LIST-61: First load fails: the error text centred, with a "Retry" button. (W/TicketListView.swift:56-57, W/TicketListView.swift:248-257)
- LIST-62: A later reload that fails keeps the list and shows a warning row at the top: "Couldn't refresh: <error>". It clears on the next successful reload. (W/TicketListModel.swift:153, W/TicketListModel.swift:162, W/TicketListView.swift:75-79)
- LIST-63: Team tickets with no teams: "You're not in any iTop team, so there are no team tickets to show." (no Retry, no controls). (W/TicketListView.swift:59-60, W/TicketListModel.swift:130)
- LIST-64: Toolbar "Refresh" button (`arrow.clockwise`), ⌘R, tooltip "Refresh (⌘R)", disabled while a reload runs. Applies to the shown list (My or Team). (W/TicketListView.swift:25-34)
- LIST-65: A reload requested while one is running is not dropped: exactly one more reload runs right after. (W/TicketListModel.swift:139-147)
- LIST-66: After a reload, a selection that no longer resolves to a ticket (in the list or the lookup) is cleared. (W/TicketListModel.swift:155)
- LIST-67: After each reload, time totals are refreshed for the loaded tickets; after a My tickets reload the timer checks its ticket (TMR-14). (W/TicketListModel.swift:156-160)
- LIST-68: Footer bar: the pager (when loaded), then a mini spinner while a reload runs (after the first load), and "Updated just now" (< 1 min), "Updated <n> min ago", "Loading" (before any load, while loading) or "Not updated yet". The text re-evaluates every 30 s. (W/TicketListView.swift:259-280)
- LIST-69: Above the list, below the lookup bar, two notice lines can show (only when the lookup is idle or found): the timer's notice (TMR-20) and then the list's notice (one line about the last action, e.g. "R-000412 is assigned to you."), in warning colour when it's an error, each with an × (tooltip/a11y "Dismiss"). These bars belong to the list view, so they're not visible while the list shows an empty/loading/error message. (W/TicketListView.swift:107, W/TicketListView.swift:201-236)
- LIST-70: The list keeps its own selection, search, page, notice, drafts and "working" flag; My tickets and Team tickets don't share them. (W/TicketListModel.swift:85-97, W/TicketListModel.swift:207-209)

---

## 4. Ticket detail

### Loading
- DET-1: The detail always shows the list's latest copy of the ticket (so an assign or reload shows at once), falling back to the copy it opened with. (W/TicketDetailView.swift:25)
- DET-2: The detail reloads from iTop when it opens and whenever the ticket's status, agent, team or last-update changes. (W/TicketDetailView.swift:63-66)
- DET-3: Detail read: `core/get` by key with description, public_log, private_log, functionalcis_list, service_id(+_friendlyname), servicesubcategory_id(+_friendlyname), last_update. No object back → error "<ref> wasn't returned. It may have been deleted, or your account can't see it any more." (K/TicketDetail.swift:44-46, K/TicketDetail.swift:73-80)
- DET-4: First load: small spinner + "Loading the ticket from iTop" under the header and strip. (W/TicketDetailView.swift:33-38)
- DET-5: First load fails: "Couldn't load this ticket. <error>" in warning colour + "Retry". A failed **re**load keeps the loaded content and says nothing. (W/TicketDetailView.swift:39-44, W/TicketDetailModel.swift:32-38)
- DET-6: The header and details strip show immediately (from the list row); service line, description, logs and the composer appear once loaded. (W/TicketDetailView.swift:30-61)

### Header
- DET-7: First line (12 pt secondary): the ref as a plain monospace button (tooltip "Copy <ref>", a11y "<ref>, copy ref"), the class icon (`questionmark.bubble` "User request" / `exclamationmark.triangle` "Incident", as tooltip and a11y), the status label, and a teal "Needs you" capsule when it needs you. (W/TicketDetailView.swift:80-105)
- DET-8: Status label: escalated_ttr "Escalated TTR", escalated_tto "Escalated TTO", waiting_for_approval "Waiting for approval", else first letter uppercased and underscores → spaces ("assigned" → "Assigned"). (W/TicketListView.swift:386-393)
- DET-9: Title 22 pt semibold, wraps, selectable. (W/TicketDetailView.swift:106-109)
- DET-10: Clicking the ref copies it to the clipboard (plain string) and shows a chip "✓ Copied <ref>" under the title for 2 s. (W/TicketDetailView.swift:81-88, W/TicketDetailView.swift:110-116, W/TicketDetailModel.swift:98-103)
- DET-11: Actions row: timer button, primary action, "…" More menu, all 26 pt high. They sit to the right of the title, or on their own row under it when the header is narrower than actions width + 12 + 260 pt. (W/TicketDetailView.swift:73-78, W/TicketDetailView.swift:118-165, W/Theme.swift:20)

### Primary action (by status)
- DET-12: Allowed actions per status: new / escalated_tto → assign; assigned / escalated_ttr → reassign, pending, resolve; pending → reassign, resolve; resolved → reopen, close; anything else (closed, waiting_for_approval, …) → none. (K/TicketLifecycle.swift:22-30)
- DET-13: Primary = first of assign, resolve, close that's allowed. So new/TTO → assign; assigned/TTR/pending → resolve; resolved → close; others → no button. (K/TicketLifecycle.swift:34-38)
- DET-14: Assign + unassigned → "Assign to me" (tooltip "Assign <ref> to you"), assigns directly. Assign + already has an agent → "Assign ticket" (tooltip "Choose a team and agent"), opens the Assign sheet. (W/TicketDetailView.swift:191-198)
- DET-15: Resolve → "Resolve ticket" (tooltip "Resolve <ref>"), opens the Resolve sheet. (W/TicketDetailView.swift:199-200)
- DET-16: Close → "Close ticket" (tooltip "Close <ref>"), asks first: dialog "Close <ref>?", message "A closed ticket can't be reopened from the app.", buttons "Close ticket" and "Cancel". (W/TicketDetailView.swift:201-202, W/TicketDetailView.swift:175-180)
- DET-17: The primary button is teal (with near-black text in dark mode, white in light) only when the ticket needs you; Close is never teal; otherwise it's the neutral near-black/near-white fill. (W/TicketDetailView.swift:187, W/TicketDetailView.swift:215, W/Theme.swift:12-23)
- DET-18: The primary is disabled while any change is running in this list or when writes aren't allowed for the ticket; then its tooltip is "Changes to iTop are switched off in this build". A mini spinner shows in it while a change runs on this ticket. (W/TicketDetailView.swift:188-189, W/TicketDetailView.swift:208-218)

### More menu ("…")
- DET-19: Icon-only `ellipsis` button, tooltip and a11y "More actions". (W/TicketDetailView.swift:145-159)
- DET-20: Assign item, shown when the state can be assigned/reassigned **and** it isn't the case "primary is Assign and the ticket already has an agent": "Assign to someone else…" when unassigned, else "Reassign…". Opens the Assign sheet. (W/TicketDetailView.swift:125-128)
- DET-21: "Set to pending…" when the state allows pending (assigned, escalated_ttr). Opens the Pending sheet. (W/TicketDetailView.swift:129-131)
- DET-22: "Reopen ticket" when the state allows reopen (resolved). Runs at once, no sheet, no confirmation; an error goes to the list's notice line. (W/TicketDetailView.swift:132-134, W/TicketListModel.swift:308-310)
- DET-23: Those three are disabled while a change runs or when writes aren't allowed. A divider follows them when the state has any action. (W/TicketDetailView.swift:124, W/TicketDetailView.swift:135)
- DET-24: Always: "Copy ref <ref>" (same as clicking the ref, with the 2 s chip) and "Open in iTop on the web", which opens `<base>/pages/UI.php?operation=details&class=<Class>&id=<key>` in the default browser. (W/TicketDetailView.swift:136-144, W/TicketListModel.swift:348-354)

### Details strip
- DET-25: A rounded tinted box that wraps its items onto new lines: caller (`person`, tooltip "Caller"), organization (`building.2`, "Organization"), team (`person.2`, "Team"; "No team" when empty), the agent, time spent, then linked CIs. Empty caller/org show "None". a11y "<Caller|Organization|Team>: <value or none>". (W/TicketDetailView.swift:222-252)
- DET-26: Agent: unassigned → dashed 18 pt circle + bold "Unassigned" (a11y "Agent: unassigned"); yours → your initials avatar + "You"; else the agent's initials (first letters of the first two words) + name. Tooltip "Agent". (W/TicketDetailView.swift:255-282)
- DET-27: Time spent item "<total> spent" (stopwatch, tooltip "Time spent") only when the known total is > 0. (W/TicketDetailView.swift:228-230)
- DET-28: Linked CIs (after loading): underlined monospace label with `server.rack`, tooltip "Linked CI: open <name> in Config items"; click opens it in Config items. CI name = `functionalci_id_friendlyname`, else `functionalci_name`, else "CI <id>"; class from `functionalci_id_finalclass_recall`, else "FunctionalCI". (W/TicketDetailView.swift:231-241, K/TicketDetail.swift:51-58)
- DET-29: Caller and organization in the strip are plain text, not links. (W/TicketDetailView.swift:224-225)
- DET-30: Under the strip (once loaded), when the ticket has a service: "<Service> › <Subcategory>" (subcategory omitted when empty), 12 pt secondary. (W/TicketDetailView.swift:46-50)

### Description and attachments
- DET-31: "Description" heading, then the description as rich text (section 4b); "No description." when the plain text is empty and the HTML has no `<img`. (W/TicketDetailView.swift:286-298)
- DET-32: Attachments are listed under the description: `SELECT Attachment WHERE item_class = '<Class>' AND item_id = <key>`, names from friendlyname (fallback "Attachment <id>"), sorted naturally. Nothing shows when there are none. (K/RichText.swift:273-280, W/RichTextView.swift:139-170)
- DET-33: With attachments: heading "Attachments" and one chip per file (paperclip icon + name, tooltip "Download <name>"). Clicking downloads the contents through REST, then opens a Save panel prefilled with the file's name (new folders allowed); after saving: "Saved <filename> (<size>)." Cancelling the panel saves nothing and shows nothing. While one downloads, its chip shows a spinner and all chips are disabled. (W/RichTextView.swift:141-163, W/RichTextView.swift:177-193)
- DET-34: Download failure: "Couldn't download <name>. <error>" (warning). Listing failure: "Couldn't list attachments. <error>" (secondary). (W/RichTextView.swift:164-165, W/RichTextView.swift:191)

### Logs
- DET-35: A segmented control "Public log" / "Private log" (text only); starts on Public each time a ticket is opened; the composer writes to the selected log. (W/TicketDetailView.swift:302-309, W/TicketDetailModel.swift:17)
- DET-36: Empty: "No public entries yet." / "No private entries yet. Only agents can see the private log." (W/TicketDetailView.swift:313-316)
- DET-37: Each entry is a bordered card: a header line "<author> · <date>" (parts left out when empty), with a lock icon (a11y "Private") on private entries, then the entry as rich text. (W/TicketDetailView.swift:317-331)
- DET-38: Entries are shown oldest first. iTop sends newest first; entries with the same timestamp are ordered by reversed position. Entries with neither `message_html` nor `message` are skipped; author = `user_login`. (K/CaseLog.swift:25-39)
- DET-39: Entry dates: same day "Today HH:mm", previous day "Yesterday HH:mm", 2–6 days ago "<weekday short> HH:mm" (e.g. "Fri 16:10"), earlier this year "<d MMM> HH:mm" ("2 Nov 09:00"), other years "d MMM yyyy" with no time. 24-hour time always. (K/CaseLog.swift:98-119)

### 4b. Rich text (descriptions and log entries)
- DET-40: HTML is never rendered by a web engine; it is parsed into text runs and image blocks, and nothing remote is fetched by the parser. (K/CaseLog.swift:44-46, K/RichText.swift:50-108)
- DET-41: `<script>`/`<style>` with content removed; `<br>` → line break; `<li>` → "\n• "; p, h1–h6, blockquote, figure, ul, ol, table, pre → blank line; div, tr, section, article, header, footer → line break; td/th → space; other tags dropped; entities decoded (numeric, hex and a fixed named list). (K/RichText.swift:52, K/RichText.swift:88-92, K/CaseLog.swift:72-92)
- DET-42: Whitespace clean-up: NBSP → space, each line trimmed, at most one blank line in a row, outer whitespace trimmed. (K/CaseLog.swift:62-70)
- DET-43: Links: only http, https and mailto become clickable (underlined); relative hrefs resolve against the server base URL; other schemes keep their text without a link. Links open in the default browser/mail app. (K/RichText.swift:93-101, K/RichText.swift:177-187, W/RichTextView.swift:56-60)
- DET-44: Pasted iTop images (`src` with `operation=download_inlineimage`): loaded by id through REST (`SELECT InlineImage WHERE id = <n>`, base64 `contents.data`), id from `data-img-id` or the `id` query parameter; never through the web link. (K/RichText.swift:189-201, K/RichText.swift:259-270)
- DET-45: `data:image/...;base64,` images are shown from the HTML directly. (K/RichText.swift:191-193)
- DET-46: Images from any other http(s) host are never loaded: if they declare width and/or height (attribute or inline style, px) all ≤ 40 they become an inline grey photo symbol; otherwise an inline chip "[photo symbol] <host> ↗" that opens the image URL in the browser (host fallback "another site"). (K/RichText.swift:77-82, K/RichText.swift:158-168, K/RichText.swift:204-209, W/RichTextView.swift:45-55)
- DET-47: Loaded images show at most 640 pt wide (never upscaled past their own width) and 480 pt high, rounded corners; tooltip "Open full size", a11y "Image, open full size". While loading: spinner + "Loading image". Failure: warning icon + "Couldn't load this image." + link "Open in iTop on the web" (to the ticket). (W/RichTextView.swift:80-104)
- DET-48: The last 60 loaded inline images are cached in memory. (K/RichText.swift:251-269)
- DET-49: Clicking an image opens a viewer sheet sized to the image within 1100 × 720 (at least 480 × 300), image fitted; a checkbox "Actual size" (tooltip "Show the image at 100%, scrolling if it's bigger than the window") switches to 1:1 with scrolling; a "W × H" pixel label; "Close" button (Esc). (W/RichTextView.swift:197-229)
- DET-50: Plain text of HTML (used for "No description" and search-free display) turns `<img>` into "[Image]". When the rich parse yields nothing, the plain text is shown. (K/CaseLog.swift:47-58, W/RichTextView.swift:16-19)

---

## 5. Log composer (incl. images)

- LOG-1: Shown pinned at the bottom of the detail only after the detail has loaded. (W/TicketDetailView.swift:60-62)
- LOG-2: Writes to the log of the selected tab. Placeholder "Write a public entry, the caller sees it" / "Write a private entry, only agents see it"; a11y "Public entry" / "Private entry". (W/TicketDetailView.swift:390, W/TicketDetailView.swift:417-428)
- LOG-3: Plain-text editor 64 pt high, 13 pt system font, no smart quotes or dashes, undo on. Tab / Shift-Tab move focus to the next/previous control instead of typing a tab. (W/ComposerTextView.swift:105-121, W/ComposerTextView.swift:175-181)
- LOG-4: Drafts are kept per ticket **and** per log (`"<ticketID>|public"` / `"<ticketID>|private"`), per list, across ticket switches and failed sends; in memory only (lost on quit). (W/TicketDetailView.swift:391-395, W/TicketListModel.swift:91-94)
- LOG-5: Send button: paperplane + "Add to public log" / "Add to private log", ⌘↩, tooltip "Add the entry (⌘↩)" (or "Changes to iTop are switched off in this build"). Enabled only when writes are allowed, nothing is sending, and the draft has non-blank text or at least one image. Spinner while sending. (W/TicketDetailView.swift:470-493, W/TicketDetailView.swift:396-399)
- LOG-6: Text-only entry: `core/update` with `<public_log|private_log>: {add_item: {message: <trimmed text>, format: "text"}}`. (K/TicketWrites.swift:38-43)
- LOG-7: On success the draft and images for the key that was sent are cleared (even if you switched tabs meanwhile) and the detail reloads. (W/TicketDetailView.swift:471-479, W/TicketDetailModel.swift:54-56)
- LOG-8: On failure the draft and images stay, and the status line says: "Write something or add an image first." (empty), "An entry can have up to 5 images. Remove some, then add the entry. Your draft is kept." (too many), "Changes to iTop are switched off in this build, so nothing was sent. Your draft is kept." (writes off), else "iTop didn't save the entry. <reason, ending in . ! or ?> Your draft is kept." (W/TicketDetailModel.swift:57-67)
- LOG-9: Status line (left, after the paperclip) priority: the error; else, while sending with images, "Uploading the image to iTop" / "Uploading <n> images to iTop"; else, when writes are off, "Adding entries is switched off in this build, so nothing is sent to iTop." A long line wraps; it never squeezes the send button. (W/TicketDetailView.swift:460-468, W/TicketDetailView.swift:492)
- LOG-10: When writes are off the editor isn't editable. (W/TicketDetailView.swift:417, W/ComposerTextView.swift:135)
- LOG-11: The entry box's border darkens while focused. (W/TicketDetailView.swift:445-446)

### Images
- LOG-12: Three ways in: paste (⌘V or Edit ▸ Paste) into the entry, drag and drop onto the entry, or the paperclip button (open panel, images only, multiple). (W/ComposerTextView.swift:161-167, W/ComposerTextView.swift:195-218, W/TicketDetailView.swift:449-459, W/TicketDetailView.swift:498-509)
- LOG-13: Paperclip tooltip: "Add images. You can also paste or drop them in the entry. PNG, JPEG or GIF, up to 4 MB each."; at 5 images "An entry can have up to 5 images"; writes off "Adding images is switched off". a11y "Add images". Disabled when writes are off, while sending, or at 5 images. (W/TicketDetailView.swift:456-459, W/TicketDetailView.swift:400)
- LOG-14: Paste rule: image file URLs on the clipboard win (Finder copy); file URLs with no images → normal paste; if the clipboard has text → normal text paste; otherwise PNG, then TIFF data (a screenshot), named "Pasted image.png" / "Pasted image.tiff". Edit ▸ Paste stays enabled when the clipboard holds images. (W/ComposerTextView.swift:57-77, W/ComposerTextView.swift:169-172)
- LOG-15: Drop rule: same, but image data is taken even when text is present. While an image is dragged over the box, its tint and border darken (2 pt border). (W/ComposerTextView.swift:195-218, W/TicketDetailView.swift:444-446)
- LOG-16: Files over 40 MB are not read at all and count as too large. (W/ComposerTextView.swift:19, W/ComposerTextView.swift:62-65, W/TicketDetailView.swift:503-506)
- LOG-17: The type is judged from the bytes: PNG (`89 50 4E 47 0D 0A 1A 0A`), JPEG (`FF D8 FF`), GIF (`GIF87a`/`GIF89a`). Any of those up to 4 MB (4 × 1 048 576 bytes) is taken as is. (K/LogImage.swift:19, K/LogImage.swift:27-47)
- LOG-18: Anything else the OS can decode is converted: TIFF (`II*\0` / `MM\0*`) → PNG, everything else (HEIC…) → JPEG at quality 0.85; scaled so the longest side is at most 2560 px; the name keeps its base name with `.png`/`.jpg` ("Pasted image" when the name is empty). The converted file must still be ≤ 4 MB. (W/ComposerTextView.swift:17, W/ComposerTextView.swift:21-53)
- LOG-19: Intake messages (joined with spaces, shown in the status line; a clean intake clears the line): "<name> is larger than 4 MB."; "<name or That file> isn't an image iTop Wrap can add. Use PNG, JPEG or GIF."; over the 5-image limit: the first ones up to the limit are kept and "An entry can have up to 5 images, so <n> wasn't added." / "…<n> weren't added." (W/TicketDetailModel.swift:73-93)
- LOG-20: Thumbnails: 56 pt squares (cropped to fill) in a horizontal row under the text, each with a small dark × (tooltip/a11y "Remove <filename>"), tooltip and a11y "<filename>, <size>" (file-style byte count). The row's a11y is "1 image to add" / "<n> images to add". Removing clears the status-line error. × disabled while sending. (W/TicketDetailView.swift:430-441, W/TicketDetailView.swift:407-411, W/TicketDetailView.swift:514-565)
- LOG-21: Images are kept per draft key, like text. (W/TicketDetailView.swift:395, W/TicketDetailView.swift:404)
- LOG-22: With images, everything is checked first (≤ 5 images, writes allowed); then each image is uploaded in order as an `InlineImage`: `item_class`, `item_id`, `item_org_id`, a 32-hex-char random `secret`, `contents: {data: base64, mimetype, filename}`, output fields `id,secret`. iTop's secret is used when it returns one. A refused upload stops everything and no entry is added (earlier uploaded images stay in iTop). (K/LogImage.swift:71-113)
- LOG-23: Then one `add_item` with `format: "html"`: the text as one `<p>` (escaped `& < > " '`, CRLF→LF, lines joined with `<br>`, nothing if blank) followed per image by `<p><img src="<base>/pages/ajax.document.php?operation=download_inlineimage&amp;id=<id>&amp;s=<secret>" width="W" height="H" style="aspect-ratio:W/H" data-img-id="<id>" data-img-secret="<secret>"></p>`; width capped at 800 px with height scaled (rounded); size attributes omitted if the pixel size is unknown. (K/LogImage.swift:129-166, K/LogImage.swift:58-64)

---

## 6. Assign sheet

- ASG-1: "Assign to me" (row pill, header button or sheet): one change at a time per list (a second click while one runs does nothing); clears the list notice; picks the ticket's own team if you're in it, else your first team; no teams → error. (W/TicketListModel.swift:221-226, K/TicketWrites.swift:66-67)
- ASG-2: It first re-reads the ticket (`agent_id`, `agent_id_friendlyname`, `status`, limit 1); if someone else now has it, it fails with "<name> took <ref> a moment ago, so it stays with them." (name fallback "someone else") and the list reloads. It uses the freshly read status to pick the stimulus. (K/TicketWrites.swift:69-85, W/TicketListModel.swift:233, W/TicketListModel.swift:318-319)
- ASG-3: Stimulus: `ev_assign` when the state allows assign (new, escalated_tto), else `ev_reassign` (assigned, escalated_ttr, pending); none for resolved/closed → "<Status> tickets can't be assigned." Fields `team_id`, `agent_id` (must be plain digits, else "Pick a team and an agent first."). (K/TicketWrites.swift:47-62, W/TicketListModel.swift:322-325)
- ASG-4: Success notice "<ref> is assigned to you." then a list reload. (W/TicketListModel.swift:229-230)
- ASG-5: No-team error text: "You aren't in any team in iTop, so Assign to me can't pick one. Choose a team and agent instead." (W/TicketListModel.swift:320-321)
- ASG-6: Sheet (380 pt wide): title "Assign <ref>" when unassigned, else "Reassign <ref>"; the ticket title under it (2 lines). (W/AssignSheet.swift:22-26, W/AssignSheet.swift:84)
- ASG-7: A full-width filled "Assign to me" at the top; disabled while any change runs or when the ticket is already yours (tooltip "It's already yours", else "Assign <ref> to you"). On success the sheet closes; on failure the sheet shows the notice text as its error. (W/AssignSheet.swift:28-38)
- ASG-8: Then centred "or choose a team and agent", a "Team" dropdown and an "Agent" dropdown. (W/AssignSheet.swift:40-55)
- ASG-9: Teams = every active team in iTop (`SELECT Team WHERE status = 'active'`, friendlyname, fallback "Team <id>"), sorted naturally; the ticket's current team is preselected. Placeholders: "Loading teams…", "No teams found", "Couldn't load: <error>", else "Choose…". Disabled until loaded. (W/AssignSheet.swift:45-49, W/AssignSheet.swift:100-117, K/TicketWrites.swift:119, K/TicketWrites.swift:136-140)
- ASG-10: Agents = active people linked to the chosen team (`SELECT Person AS p JOIN lnkPersonToTeam AS l ON l.person_id = p.id WHERE l.team_id = <id> AND p.status = 'active'`), sorted; your own entry reads "<name> (you)". Placeholders: "Choose a team first", "Loading agents…", "Nobody in this team", "Couldn't load: <error>", "Choose…". Disabled with no team or while loading. (W/AssignSheet.swift:50-55, K/TicketWrites.swift:122-126, K/TicketWrites.swift:142-147)
- ASG-11: Changing the team clears the agent and reloads agents; a late answer for a previous team is ignored. (W/AssignSheet.swift:87-90, W/AssignSheet.swift:119-128)
- ASG-12: Buttons: "Cancel" (Esc, unless a dropdown is open) and "Assign ticket" / "Reassign ticket" (⌘↩), disabled while a change runs or until both team and agent are picked; spinner while running. (W/AssignSheet.swift:61-81)
- ASG-13: Success: notice "<ref> is assigned to <you|agent name> in <team>.", list reload, sheet closes. Failure: the sheet stays with "iTop didn't assign <ref>. <reason>" (or a mapped message, ERR-14). A second assign while one runs returns "Another assignment is still running." (W/TicketListModel.swift:249-264)

---

## 7. Pending / Resolve / Reopen / Close

### Shared
- LC-1: Every lifecycle change first checks the state allows it (else "<Status> tickets can't do that. Refresh to see the ticket as it is now.") and the required fields; nothing is sent for a change iTop would refuse. (K/TicketLifecycle.swift:76-79, W/TicketListModel.swift:328-329)
- LC-2: Sent as `core/apply_stimulus` with `ev_pending` / `ev_resolve` / `ev_reopen` / `ev_close`; the per-object result code is checked too (the call can succeed while the object is refused). (K/TicketLifecycle.swift:7-16, K/TicketLifecycle.swift:97-99, K/TicketWrites.swift:92-96)
- LC-3: One change at a time per list: a second one returns "Another change is still running." The list notice is cleared when one starts. (W/TicketListModel.swift:267-271)
- LC-4: **Set to pending and Resolve stop the timer on that ticket and save its time before the change is sent**, with the notice prefix "Setting <ref> to pending stopped its timer." / "Resolving <ref> stopped its timer.". The timer stays stopped even if iTop then refuses the change. (W/TicketListModel.swift:273-278)
- LC-5: Success notices: "<ref> is set to pending." / "is resolved." / "is reopened." / "is closed.", then a list reload. (W/TicketListModel.swift:281-293)
- LC-6: Failures: "iTop didn't <set <ref> to pending | resolve <ref> | reopen <ref> | close <ref>>. <reason>" (reason gets a final "." if it lacks . ! ?), or the mapped messages in ERR-14. (W/TicketListModel.swift:295-304, W/TicketListModel.swift:316-337)
- LC-7: Sheets: title (15 pt semibold), subtitle = ticket title (2 lines), one column, buttons bottom right; 420 pt wide (New ticket 480). "Cancel" = Esc (closes an open dropdown first instead); the confirm button = ⌘↩, shows a mini spinner while running, disabled while any change runs in the list. Cancel discards typed text without asking. (W/LifecycleSheets.swift:154-178, W/LifecycleSheets.swift:235-252, W/DropdownField.swift:80-86)
- LC-8: Required labels show "Required" next to the label from the start, turning warning-red only after a confirm attempt with the field empty. (W/LifecycleSheets.swift:181-197)

### Set to pending
- LC-9: Title "Set <ref> to pending". Field "Reason" (Required); text box 90 pt, placeholder "What are you waiting for? For example the caller's reply or a part.", focused on open; a11y "Reason, required". (W/LifecycleSheets.swift:17-21)
- LC-10: Empty (whitespace) reason on confirm: "Enter why the ticket is waiting." under it; nothing sent. (W/LifecycleSheets.swift:21, W/LifecycleSheets.swift:33-34)
- LC-11: If the timer runs on this ticket: stopwatch + "Your timer stops and saves <h:mm> to this ticket." (updates every 30 s). (W/LifecycleSheets.swift:22-28)
- LC-12: Confirm "Set to pending" sends `pending_reason` = trimmed reason; success closes the sheet; failure shows the error under the form and keeps the text. (W/LifecycleSheets.swift:32-37, K/TicketLifecycle.swift:82-85)

### Resolve
- LC-13: Title "Resolve <ref>". Fields: "Service" (Required), "Service subcategory" (Required), "Resolution code" (not required), "Solution" (Required). No time field. (W/LifecycleSheets.swift:64-79)
- LC-14: Services offered: the services of the ticket's org's customer contracts (`SELECT Service AS s JOIN lnkCustomerContractToService AS l ON l.service_id = s.id JOIN CustomerContract AS cc ON l.customercontract_id = cc.id WHERE cc.org_id = <org> AND s.status != 'obsolete'`), de-duplicated; if none (or no org), every service with status ≠ obsolete. Sorted naturally by friendlyname. (K/TicketLifecycle.swift:118-145, K/TicketLifecycle.swift:152-159)
- LC-15: Prefill: the ticket's current service and subcategory (from the loaded detail, or read now if the sheet opened before the detail loaded). The current service is inserted at the top if it's not offered (named "Service <id>" if it has no name); the subcategory is kept and inserted at the top of its list if missing ("Subcategory <id>"). Ids must be digits and not "0" to prefill. (W/LifecycleSheets.swift:114-136, W/LifecycleSheets.swift:138-148)
- LC-16: Changing the service (after the prefill) clears the subcategory and reloads subcategories (`SELECT ServiceSubcategory WHERE service_id = <id> AND status != 'obsolete'`); a late answer for another service is ignored. (W/LifecycleSheets.swift:99-103, K/TicketLifecycle.swift:125-128)
- LC-17: Placeholders: service "Loading services…", "No services found", "Couldn't load: <error>", "Choose…"; subcategory "Choose a service first", "Loading subcategories…", "This service has no subcategories", "Couldn't load: <error>", "Choose…". (W/LifecycleSheets.swift:66-73, W/LifecycleSheets.swift:106-112)
- LC-18: Resolution codes are a fixed list, shown capitalised: Assistance, Bug fixed, Hardware repair, Other, Software patch, System update, Training (sent lowercase: `assistance`, `bug fixed`, …). Preselected: Assistance. (K/TicketLifecycle.swift:164-175, W/LifecycleSheets.swift:54)
- LC-19: Solution box 110 pt, placeholder "What fixed it, in words the caller understands.", focused on open; a11y "Solution, required". Empty on confirm: "Describe what fixed it. The caller sees this." (W/LifecycleSheets.swift:76-79)
- LC-20: The same timer line as LC-11 when the timer runs on this ticket. (W/LifecycleSheets.swift:80-86)
- LC-21: Confirm "Resolve ticket" does nothing visible beyond the red labels/errors unless service, subcategory, code and solution are all set. Sends `service_id`, `servicesubcategory_id`, `resolution_code`, and `solution` as HTML: markup escaped (`& < > "`), split into `<p>` paragraphs on blank lines, single line breaks as `<br>`. (W/LifecycleSheets.swift:90-95, K/TicketLifecycle.swift:86-93, K/NewTicket.swift:33-39)
- LC-22: A resolved ticket that was selected stays open under "Resolved just now" (LIST-30). (W/TicketListModel.swift:289-292)

### Reopen / Close
- LC-23: Reopen and Close have no sheet; their errors go to the list's notice line in warning colour. Close asks first (DET-16); Reopen doesn't. (W/TicketDetailView.swift:132-134, W/TicketDetailView.swift:175-180)
- LC-24: A closed ticket has no actions in the app (no primary, no lifecycle items). (K/TicketLifecycle.swift:28)

---

## 8. New ticket

### Opening
- NEW-1: File ▸ "New Ticket…" (⌘N, only when signed in) brings the main window forward and opens New ticket as a sheet there (plain start). (W/App.swift:24-27, W/App.swift:114-126)
- NEW-2: The menu bar panel's "New ticket" (⌘N) opens a separate small window titled "New Ticket", centred, without opening the main window, and activates the app. That window isn't in the Window menu, isn't restored at launch and never opens on its own. If there's no session it closes itself. (W/App.swift:41-49, W/App.swift:56-70, W/App.swift:99-102)
- NEW-3: From an org: prefilled with that org. From a CI: subtitle "On <CI name>", the CI is linked to the new ticket, and the org is matched by the CI's org name. (W/ShellView.swift:58-59, W/ShellView.swift:70, W/NewTicketView.swift:68)
- NEW-4: After creating from the separate window it closes; the new ticket is opened in My tickets for when you next open the main window. (W/App.swift:54-66)

### Form
- NEW-5: Title "New ticket". A segmented "User request" / "Incident" (default User request). (W/NewTicketView.swift:68-75)
- NEW-6: "Organization" (Required): searchable dropdown, prompt "Search organizations", no match "No organization matches"; active orgs (`SELECT Organization WHERE status = 'active'`), sorted. Placeholders "Loading organizations…", "No organizations found", "Couldn't load: <error>", "Choose…". (W/NewTicketView.swift:77-81, K/NewTicket.swift:83, K/NewTicket.swift:93-95)
- NEW-7: "Caller" (Required): searchable, prompt "Search people in <org>", no match "No caller in <org> matches"; active people of the chosen org (`SELECT Person WHERE org_id = <org> AND status = 'active'`). Placeholders "Choose an organization first", "Loading people…", "Nobody in <org>", "Couldn't load: <error>", "Choose…". (W/NewTicketView.swift:82-87, K/NewTicket.swift:85-87)
- NEW-8: "Service" (Required) and "Subcategory" (Required) side by side. Services: the org's contracted services, or all non-obsolete ones when it has none (as LC-14). Placeholders service "Choose an organization first", "Loading…", "No services"; subcategory "Choose a service first", "Loading…", "None for this service"; plus "Couldn't load: <error>" / "Choose…". (W/NewTicketView.swift:88-101, K/NewTicket.swift:111-115)
- NEW-9: Callers and services load together when the org changes; changing the org clears caller, service and subcategory; changing the service clears the subcategory. Late answers for a previous parent are ignored. (W/NewTicketView.swift:143-152, W/NewTicketView.swift:196-234)
- NEW-10: "Impact" (fixed list, default "A department"): A department (1), A service (2), A person (3). "Urgency" (default "Low"): Critical (1), High (2), Medium (3), Low (4). Not required. (K/NewTicket.swift:89-91, W/NewTicketView.swift:55-56, W/NewTicketView.swift:102-111)
- NEW-11: "Title" (Required), single-line field, placeholder "What's wrong, in a few words". "Description" (Required), 100 pt box, placeholder "What the caller reports, and anything you've already checked." (W/NewTicketView.swift:112-118)
- NEW-12: Checkbox "Assign to me", default on, remembered across launches (UserDefaults `newTicket.assignToMe`). (W/NewTicketView.swift:44, W/NewTicketView.swift:119-121)
- NEW-13: When ticket creation isn't allowed (debug builds): "Creating tickets is switched off in this build, so nothing is sent to iTop." and "Create ticket" is disabled. (W/NewTicketView.swift:124-127, W/NewTicketView.swift:139)

### Validation and create
- NEW-14: On "Create ticket" (⌘↩) with something missing: the first missing field, in the order organization, caller, service, subcategory, title, description, is named: "Fill in the <field> first." and the empty fields' "Required" turns red. Nothing is sent. (W/NewTicketView.swift:122, W/NewTicketView.swift:155-163, W/NewTicketView.swift:265-268)
- NEW-15: Sends `core/create` on UserRequest/Incident with org_id, caller_id, service_id, servicesubcategory_id, impact, urgency, trimmed title, and description as HTML (LC-21 rules); with a CI, `functionalcis_list: [{functionalci_id: <id>}]`. (K/NewTicket.swift:44-60)
- NEW-16: Then reads the new ticket back (`SELECT <cls> WHERE id = <key>`, list fields, limit 1). No key back → "iTop didn't return the new ticket."; read-back fails → "The ticket was created, but the app couldn't read it back. Refresh to see it." (both shown as "iTop didn't create the ticket. iTop returned error 0: …"). (K/NewTicket.swift:61-70, W/NewTicketView.swift:277-281)
- NEW-17: Errors show under the form, prefixed "iTop didn't create the ticket. " (or the mapped messages ERR-14 with "the ticket" as the ref). The form stays filled. (W/NewTicketView.swift:277-282)
- NEW-18: While creating, the button shows a spinner and is disabled. (W/NewTicketView.swift:138-139)
- NEW-19: Success: draft forgotten, sheet/window closes, sidebar switches to My tickets, the ticket is shown there as the search result (query = its ref, selected) with notice "<ref> is created.". (W/NewTicketView.swift:273-276, W/Workspace.swift:51-54, W/TicketListModel.swift:340-345)
- NEW-20: With "Assign to me" on, it's then assigned to you (ASG-1); the notice becomes "<ref> is created and assigned to you." or, on failure, "<ref> is created, but not assigned to you. <why>" (error), and the ticket stays open. (W/TicketListModel.swift:239-246; Tests/ITopWrapTests/LookupRefreshTests.swift:86-108)

### Draft
- NEW-21: Cancel (or Esc) keeps what you typed as a draft (in memory, for this session) unless org, title and description are all empty. Closing the separate window with its close button does **not** keep a draft. (W/NewTicketView.swift:133-137, W/NewTicketView.swift:250-254)
- NEW-22: The next **plain** New ticket restores the draft (type, org, caller, service, subcategory, impact, urgency, title, description) without the cascade clearing children. A start from an org or a CI starts fresh and leaves the draft untouched. (W/NewTicketView.swift:175-194, W/NewTicketView.swift:60-61)
- NEW-23: Whenever a draft exists, a "Clear form" button shows at the bottom left; it forgets the draft and resets every field (keeping the chosen type), impact/urgency back to defaults, and hides errors. (W/NewTicketView.swift:129-132, W/NewTicketView.swift:256-263)

---

## 9. Timer

### Rules
- TMR-1: One timer for the whole app, shared by the main window and the menu bar. (W/Workspace.swift:43-46)
- TMR-2: A start button exists only on tickets that are assigned to you **and** in status `assigned` exactly (not escalated_ttr, not pending). (W/TimerModel.swift:53)
- TMR-3: Starting when writes aren't allowed for the ticket: no start, error notice "Saving time to iTop is switched off in this build, so the timer can't start." (W/TimerModel.swift:68-71)
- TMR-4: Before starting it checks for your web stopwatch on that ticket; if one is running, no timer starts and the notice says "Your web stopwatch is already running on <ref> since <time>. Stop it in iTop, or keep using it there." (W/TimerModel.swift:72-78)
- TMR-5: Starting while another timer runs first stops it and saves its time (TMR-8), then starts the new one. Starting is ignored while a save is in progress. (W/TimerModel.swift:67, W/TimerModel.swift:80-84)
- TMR-6: The running timer (class, key, ref, title, org id, start time) is saved to UserDefaults `timer.running` at start and removed at stop, so it survives quitting; elapsed time keeps counting through quit and sleep (it's "now minus start"). (W/TimerModel.swift:82-83, K/TimeTracking.swift:9-27, K/TimeTracking.swift:92-113)
- TMR-7: Stop: elapsed is capped at 4 hours; under 60 seconds nothing is saved: "<prefix>The timer on <ref> ran under a minute, so nothing was saved." (W/TimerModel.swift:88-100, K/TimeTracking.swift:46-56)
- TMR-8: Saving: find the ticket's Activity (`SELECT Activity WHERE obj_class = '<Class>' AND obj_id = <key>`); if none, create one (`obj_class`, `obj_id`, `label` = "<ref> <title>"); then create a `TimeSpent` with activity_id, user_id (your User), contact_id (your Person), org_id (the ticket's), start_date, end_date (= start + duration), duration (seconds, as a string), title = the ref, origin = "stopwatch". Dates `yyyy-MM-dd HH:mm:ss` in local time. (K/TimeTracking.swift:124-156, K/TimeTracking.swift:80-88)
- TMR-9: No usable activity id after that → "iTop didn't return the ticket's time-tracking activity." (K/TimeTracking.swift:144-146)
- TMR-10: Saved: notice "<prefix>Saved <h:mm> to <ref>." plus, when capped, " It stopped at 4:00, like iTop's own stopwatch."; the shown total for that ticket goes up at once. (W/TimerModel.swift:110-115)
- TMR-11: Save failed: error notice "<prefix>Couldn't save <h:mm> to <ref>. <reason>." with a "Retry" button; the entry is kept for retry. The × on that notice discards the time (tooltip/a11y "Discard this time"). (W/TimerModel.swift:116-121, W/TicketListView.swift:207-213)
- TMR-12: Retry saves the same entry again (no prefix); disabled while saving. (W/TimerModel.swift:102-105, W/TicketListView.swift:208)
- TMR-13: The prefix (with a trailing space) explains stops you didn't ask for: "Setting <ref> to pending stopped its timer.", "Resolving <ref> stopped its timer.", "<ref> left Assigned, so its timer stopped." (W/TimerModel.swift:94, W/TicketListModel.swift:275-276, W/TimerModel.swift:128)
- TMR-14: After **every successful My tickets reload**, if the running ticket isn't in the loaded list, or is no longer assigned-to-you-and-`assigned`, the timer stops and saves with the "left Assigned" prefix. This includes the ticket falling outside the View range (e.g. "Last 7 days" and it's older). It also runs on the first load after a relaunch. (W/TimerModel.swift:125-129, W/TicketListModel.swift:157)
- TMR-15: Web stopwatch detection: with the ticket's Activity, `SELECT TimeSpent WHERE activity_id = <a> AND user_id = <me>` (start_date, end_date, duration); the last one with an empty end_date is a running stopwatch, its start_date is shown. Checked each time a ticket's detail opens (only for startable tickets not running here) and at start. (K/TimeTracking.swift:159-166, W/TimerModel.swift:142-145, W/TimerButton.swift:45)
- TMR-16: Time shown with "since": today → time only (short); other days → abbreviated date + short time. (W/TimerModel.swift:153-155)
- TMR-17: Totals: for the open (not resolved/closed) tickets of each reload, per class: Activities for those keys, then all TimeSpent for those activities (`activity_id`, `duration`), summed per ticket, **everyone's** time. Tickets with an Activity but no time get 0; without an Activity they're left out. New totals are merged over old ones; a failure keeps the old totals silently. (K/TimeTracking.swift:169-194, W/TimerModel.swift:136-140)
- TMR-18: Sign-out doesn't stop the timer (AUTH-31). There's no prompt on wake or relaunch. (W/SessionModel.swift:108-111)

### Controls
- TMR-19: Header timer button (⌘T toggles it, when it's there): running on this ticket → stop icon + live "HH:MM:SS" (bold mono), tooltip "Stop timer: saves its time (⌘T)", a11y "Stop timer"; else, web stopwatch seen → a globe label "Web stopwatch since <time>" (tooltip "Your stopwatch in iTop's web UI is running on this ticket, so the app doesn't start its own"), no button; else startable → stopwatch icon, a11y "Start timer", tooltip "Start timer (⌘T)" or, when another runs, "Start timer: stops the one on <ref> and saves its time (⌘T)", disabled (tooltip "Changes to iTop are switched off in this build") when writes are off, and while saving; else nothing. (W/TimerButton.swift:11-46)
- TMR-20: Timer notice bar above the lists (LIST-69) and in the menu bar panel (MB-9): stopwatch icon, the text, "Retry" if retryable, ×. Warning colour for errors. (W/TicketListView.swift:202-220)
- TMR-21: The pending and resolve sheets announce the stop (LC-11). (W/LifecycleSheets.swift:22-28)
- TMR-22: List rows and the details strip show totals and the running state (LIST-56, DET-27). (W/TicketListView.swift:320-332)

---

## 10. Menu bar item & panel

### Status item
- MB-1: The menu bar item exists only while signed in, whether or not the main window is open. (W/App.swift:77)
- MB-2: Label = a monochrome template icon (three stacked ticket stubs, 18 × 18, drawn in code) followed by, when present, the running timer "HH:MM:SS" and the Needs-you count, separated by two spaces (e.g. "00:12:40  3"). Nothing after the icon when no timer and count 0. Digits are fixed-width (monospaced-digit menu bar font) so it doesn't jitter. (K/MenuBarStatus.swift:36-41, W/MenuBarIcon.swift:7-49)
- MB-3: The label ticks every second while a timer runs; otherwise it updates at most every 15 s. (W/Workspace.swift:64-73)
- MB-4: When the label's text width changes (timer starts/stops, count appears/disappears), the text slides out of / back into the icon over 12 steps of 18 ms, cubic ease-out, ending exactly on the target; shrinking keeps drawing the old text while it slides in. No animation with Reduce Motion. (W/MenuBarView.swift:20-38, W/MenuBarView.swift:42-50; Tests/ITopWrapTests/MenuBarSlideTests.swift)
- MB-5: a11y label: "iTop Wrap: nothing needs you" / "iTop Wrap: 1 ticket needs you" / "iTop Wrap: <n> tickets need you", plus ", timer <h:mm> on <ref>" while running. (K/MenuBarStatus.swift:43-52)
- MB-6: Clicking it opens a panel (a popover-style window), 360 pt wide. (W/App.swift:88, W/MenuBarView.swift:107)

### Panel
- MB-7: Top row: a text field "Search my tickets" (its own query, not the main window's) and a bordered "New ticket" button (square.and.pencil, ⌘N, tooltip "New ticket (⌘N)") that opens the New Ticket window (NEW-2). (W/MenuBarView.swift:78-88)
- MB-8: The panel lists My tickets minus resolved/closed, filtered by the panel search (same word rule as LIST-16), grouped and sorted with the shared sort, keeping only the groups Escalated TTR, Escalated TTO, Assigned, Pending, in that order. **New and Other tickets don't appear.** The org filter and paging don't apply. (W/MenuBarView.swift:64-68, K/MenuBarStatus.swift:15)
- MB-9: The timer notice shows under the top row (11.5 pt) with Retry and × (a11y "Discard this time"/"Dismiss"). (W/MenuBarView.swift:89-101)
- MB-10: The running ticket, if it is among the listed tickets, moves out of its group to the top under the heading "Running"; a group it leaves empty disappears. A timer on a ticket that isn't listed changes nothing. (K/MenuBarStatus.swift:20-28, W/MenuBarView.swift:130-133)
- MB-11: Group headings "<Group>  <count>" (two spaces; the count is the group's shown tickets). (W/MenuBarView.swift:134-136)
- MB-12: Row: teal 7 pt dot if it needs you, the ref (mono 11), the title (12.5 pt medium, 1 line); clicking the ref/title area opens the main window (reopening it if closed), switches to My tickets, clears its search and selects the ticket; tooltip "Open <ref> in iTop Wrap". (W/MenuBarView.swift:156-173, W/App.swift:102-110)
- MB-13: Row right side: while running, live "HH:MM:SS" bold mono and a tinted row background; else the total when > 0 (mono, secondary). Then the timer button: stop (tooltip "Stop the timer and save its time", a11y "Stop timer on <ref>"); start when startable and no web stopwatch is known for it (tooltip "Start timer", or "Stops the timer on <ref>, saves its time, and starts one here" when another runs; a11y "Start timer on <ref>"; disabled while saving); otherwise an empty 22 pt slot so times line up. No "Assign to me" in the panel. (W/MenuBarView.swift:174-206)
- MB-14: States: loading "Loading tickets from iTop" with a spinner; failed "Couldn't load your tickets. <error>" + "Retry"; nothing to list: "Nothing waiting on you. New assignments show up here." or, with a search, "No tickets of yours match “<query>”." (W/MenuBarView.swift:111-127)
- MB-15: The list scrolls with a height of min(420, (sections + running?1:0) × 30 + rows × 48 + 6) pt. (W/MenuBarView.swift:141-144)
- MB-16: Footer: Refresh icon button (⌘R, tooltip "Refresh (⌘R)", a11y "Refresh", disabled while reloading) reloading My tickets; the middle text "Updated <relative time, named style>" (e.g. "Updated 2 minutes ago", only after a load); and an "Open iTop Wrap" icon button (`macwindow`, ⌘O, tooltip "Open iTop Wrap (⌘O)") that opens/fronts the main window without changing selection. (W/MenuBarView.swift:208-229)

---

## 11. CMDB (Config items)

- CMDB-1: Search field in the toolbar, prompt "Search config items". Idle: "Search by name, for example a server or application." (W/CMDBView.swift:24, W/CMDBView.swift:31-32)
- CMDB-2: Searches only at ≥ 2 characters (trimmed); fewer → idle. 300 ms debounce; typing cancels the pending search and restores the previous results. The answer is dropped if the query changed. Coming back with the same query doesn't search again. (W/CMDBModel.swift:75-98)
- CMDB-3: Query `SELECT FunctionalCI WHERE name LIKE '%<text>%'`, fields name, finalclass, org_id_friendlyname, limit 50; results sorted naturally by name; the class shown is `finalclass` (the real class). (K/ConfigItem.swift:121-129, K/ConfigItem.swift:164-173)
- CMDB-4: While searching: spinner + "Searching iTop". Failure: "Couldn't search iTop. <error>" (warning). No results: "No config items match “<query>”." (W/CMDBView.swift:33-42)
- CMDB-5: Results header: "1 result", "<n> results", or "First <n> results" when it hit the limit of 50. Rows: name (mono 12.5, 1 line) and "<Class label> · <Org>" (parts left out when empty). (W/CMDBView.swift:44-59)
- CMDB-6: Class label: CamelCase split into words, later words lowercased unless all-caps: "ApplicationSolution" → "Application solution", "NetworkDevice" → "Network device", "PC" → "PC". (K/ConfigItem.swift:21-37)
- CMDB-7: Org scope (from an org's "Config items" link): a bar "Config items in <org>" (building icon) with an × (tooltip "Show search instead", a11y "Clear organization") that clears it back to idle; lists that org's CIs (`SELECT FunctionalCI WHERE org_id = <id>`, first 50, sorted). Typing a search clears the scope. (W/CMDBView.swift:11-21, W/CMDBModel.swift:63-82, K/ConfigItem.swift:132-134)
- CMDB-8: Selecting a result shows it in the detail. A CI opened from a chip or a ticket can be shown even though it isn't in the results. (W/CMDBModel.swift:39-47, W/CMDBModel.swift:101-104)
- CMDB-9: Detail loads, concurrently: the CI with all fields (`output_fields: *`), "impacts" related objects up (depends on) and down (used by) with depth 1, and its open tickets. A failure of the relationship lookups shows them as empty; a failure of the CI or tickets fails the detail. (K/ConfigItem.swift:176-198)
- CMDB-10: Loading: spinner + "Loading <name> from iTop". Failure: "Couldn't load this config item. <error>" + "Retry". (W/CMDBView.swift:85-94)
- CMDB-11: Header: "<Class label> · <Status>" (status once loaded), the name 22 pt semibold monospace, selectable; buttons: bordered "New ticket" (tooltip "New ticket on <name>"), copy-name icon (tooltip/a11y "Copy name"; shows "✓ Copied <name>" for 2 s), open-in-web icon (tooltip/a11y "Open in iTop on the web", `<base>/pages/UI.php?operation=details&class=<Class>&id=<key>`). (W/CMDBView.swift:119-153, W/CMDBModel.swift:118-128)
- CMDB-12: Fields grid (adaptive columns ≥ 160 pt), in this order when present and not empty or "0": Status, Organization, Business criticality, Location, Brand, Model, OS family, OS version, Serial number, Asset number, Management IP, CPU, RAM, In production since; then Contacts (contact friendly names joined by ", "). Status and Business criticality are humanised ("not_applicable" → "Not applicable"). (K/ConfigItem.swift:85-118, W/CMDBView.swift:155-167)
- CMDB-13: "Description" (plain text of the HTML, selectable) only when not empty. (W/CMDBView.swift:97-102)
- CMDB-14: "Relationships": "Depends on" chips ("Nothing listed in the CMDB." when none) and "Used by" chips plus a sentence: "Nothing in the CMDB depends on <name>." or "If <name> fails, <A, B and C> is/are affected." (locale list join; "is" for one). Chips are mono, bordered; tooltip "<Class label>: open <name>"; clicking opens that CI. People, teams, orgs, locations, tickets, changes, documents and the CI itself are excluded from relationships. (W/CMDBView.swift:169-205, K/ConfigItem.swift:200-211)
- CMDB-15: "Open tickets on this CI <n>": rows (needs-you dot, ref, title, "<Status> · <Unassigned|You|agent>"), tooltip "Open <ref>", click → SHELL-9. None: "No open tickets on <name>." Open = not resolved/closed via `lnkFunctionalCIToTicket`, both classes. Not affected by the View menu. (W/CMDBView.swift:207-234, K/ConfigItem.swift:137-140, K/ConfigItem.swift:213-221)

---

## 12. Organizations

- ORG-1: List: every active org (`SELECT Organization WHERE status = 'active'`, name, code), sorted naturally, each with its count of **open** tickets you can see (both classes, all agents; computed by loading every open ticket's org_id; independent of the View menu). (K/Organization.swift:117-130)
- ORG-2: Loaded once per session on first visit; not refreshed after that (only Retry after a failure reloads). (W/OrgsModel.swift:67-76, W/OrgsView.swift:153)
- ORG-3: Search field prompt "Search organizations", filtering locally on "<name> <code>" (case-insensitive substring). (W/OrgsModel.swift:56-60, W/OrgsView.swift:152)
- ORG-4: States: "Loading organizations from iTop"; "Couldn't load organizations. <error>" + "Retry"; none visible: "Your iTop account can't see any organizations." or "No organization matches “<query>”." (W/OrgsView.swift:117-133)
- ORG-5: Row: name (medium), code under it when present, count on the right (tooltip/a11y "<n> open tickets"). (W/OrgsView.swift:135-148)
- ORG-6: Detail header: "Organization · <code>" (or "Organization"), name 22 pt semibold. (W/OrgsView.swift:168-171)
- ORG-7: Link-style buttons: "People <n>" and "Config items <n>" (just "People"/"Config items" until counts load or if they fail; counts = all Persons of the org, all FunctionalCIs of the org), and "New ticket" (tooltip "New ticket for <org>"). See SHELL-12. (W/OrgsView.swift:172-185, K/Organization.swift:138-143, W/OrgsModel.swift:78-90)
- ORG-8: Section heading "Open tickets" (or "Tickets" when resolved/closed are included), the View summary line under it, and the View menu on the right. (W/OrgsView.swift:186-193)
- ORG-9: Tickets = the org's tickets from every agent with the shared scope (`SELECT <cls> WHERE <scope> AND org_id = <id>`); reloaded when the org or the range/closed toggle changes. (K/Organization.swift:75-77, W/OrgsView.swift:213-214)
- ORG-10: States: "Loading <org>'s tickets"; "Couldn't load the tickets. <error>" + "Retry"; empty: "<org> has no open tickets." (default scope) or "<org> has no tickets in this range." (W/OrgsView.swift:194-204)
- ORG-11: Ticket table (shared with People): column headers "Ref", "Title", "Caller", "Assignee"; groups in Team order (LIST-12) with the group's whole count; rows: needs-you dot, ref (mono), red "!" for P1 only, title (1 line), caller, assignee chip (initials + "You"/name, or dashed circle + bold "Unassigned"); each row tooltip "Open <ref>", click → SHELL-9; the shared sort and page size; its own page, back to 1 when the tickets, sort or page size change; pager under it (LIST-49). (W/OrgsView.swift:7-107)

---

## 13. People

- PPL-1: Search field prompt "Search people by name"; idle: "Search by name, for example a caller or an agent." (W/OrgsView.swift:238, W/OrgsView.swift:245-248)
- PPL-2: Searches at ≥ 2 characters, 300 ms debounce (as CMDB-2). Every space-separated word must match `name LIKE '%w%' OR first_name LIKE '%w%'`. At most 100 people are fetched; the total comes from iTop's "Found: <n>" message. Sorted naturally by display name. (W/OrgsModel.swift:138-165, K/Organization.swift:88-96, K/Organization.swift:174-185, K/ITopClient.swift:210-214)
- PPL-3: More found than shown: a bar "Showing <shown> of <total> people. Type more of the name to narrow it down." (or, in an org scope, "… Search by name to find someone.") (W/OrgsView.swift:258-266)
- PPL-4: States: "Searching iTop"; "Couldn't search iTop. <error>"; none: "Nobody matches “<query>”." or, in an org scope, "Nobody in this organization." (W/OrgsView.swift:249-256)
- PPL-5: Org scope (from an org's "People" link): bar "People in <org>" with × (tooltip "Show search instead", a11y "Clear organization"); lists all Persons of the org (`SELECT Person WHERE org_id = <id>`, no status filter, ≤ 100); clears the selection; typing clears the scope. (W/OrgsView.swift:225-235, W/OrgsModel.swift:125-148, K/Organization.swift:79-81)
- PPL-6: Row: initials avatar (first letters of the first two words), name, subtitle "Agent · <team names, comma-separated>" or "Caller · <org>" ("Caller" alone with no org). Someone with at least one team is an agent. (W/OrgsView.swift:267-279, K/Organization.swift:41-50)
- PPL-7: Person display name = "first_name name", else "Person <id>". (K/Organization.swift:63-66)
- PPL-8: Detail: 44 pt initials avatar, name (22 pt, selectable), "Agent" or "Caller" plus " · <function>" when set. (W/OrgsView.swift:293-301)
- PPL-9: Info grid (only filled rows): "Organization" as a link to the org view; "Email" as a mailto link; "Phone" (selectable); "Teams" (agents only, comma-separated). (W/OrgsView.swift:302-323)
- PPL-10: Tabs "Assigned to them <n>" / "Reported by them <n>" (counts once loaded). On opening a person: agents start on Assigned, callers on Reported. Changing the View range keeps the current tab. (W/OrgsView.swift:325-331, W/OrgsModel.swift:168-178, W/OrgsView.swift:356)
- PPL-11: Both lists load together with the shared scope: `agent_id = <id>` and `caller_id = <id>`. (K/Organization.swift:98-104)
- PPL-12: View menu to the right of the tabs; summary line under. (W/OrgsView.swift:332-334)
- PPL-13: States: "Loading tickets"; "Couldn't load the tickets. <error>" (no Retry); empty: "No open tickets assigned to <name>." / "No open tickets reported by <name>." (or "No tickets in this range assigned to/reported by <name>." when the scope isn't plain open). (W/OrgsView.swift:337-345)
- PPL-14: Table (ORG-11) with, on Assigned, only "Ref / Title / Caller" (caller as the last column, 130 pt); on Reported, "Ref / Title / Caller / Assignee". (W/OrgsView.swift:347-349, W/OrgsView.swift:24-29)

---

## 14. Notifications

- NOTIF-1: **None.** The app posts no macOS notifications and asks for no notification permission (no UserNotifications code exists). Do not add any. (grep of Sources: no `UserNotifications`)
- NOTIF-2: All feedback is in-app: the list notice line, the timer notice, sheet errors, the sidebar and menu bar counts. (W/TicketListView.swift:201-236, W/MenuBarView.swift:89-101)

---

## 15. Menus & keyboard shortcuts

- KEY-1: File menu: the standard "New" item is replaced by "New Ticket…" ⌘N (only while signed in) and "Open iTop Wrap" ⌘O (always; opens/fronts the main window). (W/App.swift:24-29, W/App.swift:114-138)
- KEY-2: View menu, after the sidebar items, while signed in: a divider and "My tickets" ⌘1, "Team tickets" ⌘2, "Config items" ⌘3, "Organizations" ⌘4, "People" ⌘5. (W/App.swift:30-38, W/SessionModel.swift:31)
- KEY-3: ⌘R: Refresh the shown ticket list (toolbar, My/Team only); in the menu bar panel, refresh My tickets. (W/TicketListView.swift:30, W/MenuBarView.swift:212)
- KEY-4: ⌘T: start/stop the timer on the ticket shown in the detail (only when the timer button is there). (W/TimerButton.swift:44)
- KEY-5: ⌘↩: send the log entry; confirm the Assign, Pending, Resolve and New ticket sheets. (W/TicketDetailView.swift:488, W/AssignSheet.swift:79, W/LifecycleSheets.swift:250)
- KEY-6: Esc: cancel the sheet; if a dropdown is open, Esc only closes the dropdown. Esc closes the image viewer. (W/DropdownField.swift:80-86, W/RichTextView.swift:224)
- KEY-7: Return: "Sign in to iTop" on the sign-in form; "Try again" on the unreachable screen. (W/LoginView.swift:62, W/ShellView.swift:201)
- KEY-8: Menu bar panel: ⌘N New ticket, ⌘R Refresh, ⌘O Open iTop Wrap. (W/MenuBarView.swift:85, W/MenuBarView.swift:212, W/MenuBarView.swift:224)
- KEY-9: Lists: ↑/↓ move the selection and the detail follows (native list behaviour). The toolbar search fields are the system ones (Esc clears). (W/TicketListView.swift:71)
- KEY-10: Composer: Tab / Shift-Tab leave the entry box. (W/ComposerTextView.swift:175-181)

### Dropdown keyboard and behaviour (every picker)
- KEY-11: One dropdown component for every picker (Organization, Caller, Service, Subcategory, Impact, Urgency, Team, Agent, Resolution code): a 32 pt field with an up/down chevron; the list opens below the field (above when it doesn't fit), up to 260 pt tall, 26 pt rows, a check mark on the selected item, a grey highlight on the keyboard position, 8 pt radius, hairline border, shadow. (W/DropdownField.swift:137-151, W/DropdownField.swift:194, W/DropdownField.swift:210-248, W/DropdownField.swift:54-78)
- KEY-12: Only one dropdown in a sheet is open at a time; clicking outside closes it; losing focus closes it. Picking an item closes it. (W/DropdownField.swift:54-78, W/DropdownField.swift:147-148, W/DropdownField.swift:280-290)
- KEY-13: Searchable ones (Organization, Caller): a magnifier and a text field showing the selection; typing filters (every word must be in the label, case-insensitive), resets the highlight to the first match and opens the list; ↓/↑ move (opening first if closed), Return picks the highlighted one (or opens if closed), Esc closes and drops the typed text. Empty filter: the given no-match text, or "Nothing matches “<query>”." (W/DropdownField.swift:130-135, W/DropdownField.swift:164-187, W/DropdownField.swift:233-237)
- KEY-14: Short lists: the field itself takes focus; ↓/↑ open and move, Return or Space pick (or open), Esc closes, typing a letter jumps to the first item starting with it. (W/DropdownField.swift:299-305, W/DropdownField.swift:332-356)
- KEY-15: Opening highlights the current selection. A dropdown with no items or disabled can't open. Clicking the field toggles it. (W/DropdownField.swift:128, W/DropdownField.swift:199-203, W/DropdownField.swift:252-258)
- KEY-16: Border darkens while open or focused (1.5 pt when focused). (W/DropdownField.swift:141-142)
- KEY-17: a11y: short lists are one button-like element with the field's name, its value (selection or placeholder; the highlighted item while open) and hint "Use the up and down arrow keys to choose, Return to pick"; searchable ones expose the text field with hint "Type to filter, then use the up and down arrow keys and Return to pick". Moving announces the highlighted item. (W/DropdownField.swift:149-150, W/DropdownField.swift:180-182, W/DropdownField.swift:276, W/DropdownField.swift:309-329)

---

## 16. Persistence

### UserDefaults (standard domain, app bundle id `io.github.alfi-rajasyah.itopwrap`)
- PERS-1: `server` (string): the normalised base URL, set at sign-in. (K/SessionService.swift:12-15, K/SessionService.swift:75)
- PERS-2: `user` (string): the trimmed username, set at sign-in. (K/SessionService.swift:17-20, K/SessionService.swift:76)
- PERS-3: `list.range` (int): 0 any time, 7, 30, 90; unknown/missing → any time. (W/ListOptions.swift:25, W/ListOptions.swift:34)
- PERS-4: `list.includeClosed` (bool), default false. (W/ListOptions.swift:26, W/ListOptions.swift:35)
- PERS-5: `list.sort` (string): `priority`, `lastUpdate`, `newest`, `oldest`; unknown → priority. (W/ListOptions.swift:27, W/ListOptions.swift:36, K/TicketQueries.swift:29-30)
- PERS-6: `list.pageSize` (int): 10/20/30/50/100, **0 = All**; missing → 20. (W/ListOptions.swift:29, W/ListOptions.swift:37-38)
- PERS-7: `timer.running` (data): JSON of `{ticketClass, ticketKey, ref, title, orgID, start}`; `start` is encoded with Foundation's default date encoding (a number of seconds since 2001-01-01 00:00:00 UTC). Removed when the timer stops. A migration that reads existing values must decode this. (K/TimeTracking.swift:9-27, K/TimeTracking.swift:95-112)
- PERS-8: `newTicket.assignToMe` (bool), default true; always in the standard domain, even with the debug settings switch. (W/NewTicketView.swift:44)
- PERS-9: Each View setting is written the moment it changes. (W/ListOptions.swift:25-29)

### Keychain
- PERS-10: Generic password, service "iTop Wrap", account "<username>@<baseURL>", label "iTop Wrap (<account>)"; updated in place if it exists, else added. (K/KeychainStore.swift:16-50, K/SessionService.swift:61-63)

### Not persisted (in memory only, or not at all)
- PERS-11: Sidebar selection, list selections, searches (lists, menu bar panel, CMDB, orgs, people), org filter, pages, the detail's log tab, People tab, log drafts and draft images, the New ticket draft, notices, time totals, the image cache, "just now" tickets. (W/SessionModel.swift:49, W/TicketListModel.swift:85-97, W/Workspace.swift:23)
- PERS-12: The main window's size and position use the system's default window restoration; the New Ticket window is never restored. (W/App.swift:16-22, W/App.swift:46)

---

## 17. Debug builds: read-only and write allowlist

- DBG-1: Release builds: writes on for everything the iTop account may change. Debug builds: **all writes off** unless a switch names tickets. Release builds ignore every `ITOPWRAP_*` switch. (W/App.swift:184-198)
- DBG-2: `ITOPWRAP_WRITE_TICKETS=R-…,I-…` (comma-separated, trimmed, uppercased, empties dropped): at sign-in each ref is looked up and its "<Class>::<key>" added to an allowlist; refs that don't exist are left out. Only updates/stimuli on those objects, and creates that belong to them (InlineImage, Activity, TimeSpent), are sent. (W/App.swift:186-188, K/SessionService.swift:100-108, K/ITopClient.swift:70-80)
- DBG-3: `ITOPWRAP_WRITE_NEW=1`: also allow creating UserRequest/Incident; each created ticket joins the allowlist at once. Alone (no refs) it gives an empty allowlist plus creation. (W/App.swift:187-188, K/ITopClient.swift:53-59, K/NewTicket.swift:64)
- DBG-4: A refused write throws before anything is sent; it never reaches the network. (K/ITopClient.swift:88-89)
- DBG-5: UI with writes off: primary action and More items disabled (tooltip "Changes to iTop are switched off in this build"); composer read-only with "Adding entries is switched off in this build, so nothing is sent to iTop."; paperclip "Adding images is switched off"; send tooltip "Changes to iTop are switched off in this build"; row "Assign to me" pills hidden; timer start disabled (tooltip same), or notice "Saving time to iTop is switched off in this build, so the timer can't start."; New ticket "Creating tickets is switched off in this build, so nothing is sent to iTop." and Create disabled. (W/TicketDetailView.swift:188-189, W/TicketDetailView.swift:466, W/TicketDetailView.swift:458, W/TicketDetailView.swift:490, W/TicketListView.swift:183, W/TimerButton.swift:39-41, W/TimerModel.swift:68-70, W/NewTicketView.swift:124-127)
- DBG-6: A write that still gets refused shows "Changes to iTop are switched off in this build, so nothing was sent." (actions) or the composer text in LOG-8. (W/TicketListModel.swift:330-331)
- DBG-7: `ITOPWRAP_SETTINGS=<name>`: server/username/list settings/timer go to the UserDefaults suite "ITopWrap.debug.<name>", and the Keychain service becomes "iTop Wrap debug <name>". (W/App.swift:189-191, W/App.swift:214-219, W/Workspace.swift:43)
- DBG-8: `ITOPWRAP_DEMO=1`: a made-up signed-in user ("Demo Agent", login agent.demo, teams Network and Service desk, server https://itop.example.com) and canned tickets, CIs, orgs, people; nothing is sent anywhere; writes are simulated in memory. Extra: `ITOPWRAP_DEMO_MANY=1` (60 bulk tickets in Team), `ITOPWRAP_DEMO_STATE=empty|loading|error` (forced list states), `ITOPWRAP_DEMO_TIMEFAIL=1` (time saves fail). Demo triggers: "fail" in a log entry/pending reason/solution/title fails it, "refuse" with images fails the upload, R-000436 plays the "colleague took it" race, R-000405 has a running web stopwatch, R-000512 is the not-in-queue lookup. (W/App.swift:200-563)
- DBG-9: `ITOPWRAP_TRACE=1`: one stderr line per REST call (operation, class, OQL, code, message, object count) and per UI event; never credentials or field values. `ITOPWRAP_STDERR=<file>` appends stderr to a file. `ITOPWRAP_APPEARANCE=light|dark` forces the appearance. (K/ITopClient.swift:156-177, W/Theme.swift:96-108, W/App.swift:566-580)

---

## 18. Errors, offline, empty and loading states

### Error mapping
- ERR-1: Network failure (any URL error except certificate ones) → in most UI places "Can't reach <host>. Check your connection, then retry." (`<host>` = the server's host, "the server" if none). On the sign-in form and unreachable screen the generic "Can't reach the server. Check your connection and the server address." is used instead. (W/Theme.swift:85-92, W/Workspace.swift:31, K/ITopError.swift:46-47)
- ERR-2: Certificate untrusted / bad date / unknown root / not yet valid → "This Mac doesn't trust the server's certificate. Ask your iTop admin, or trust its certificate in Keychain Access." (K/ITopClient.swift:192-200, K/ITopError.swift:48-49)
- ERR-3: HTTP status outside 200–299 → "The server answered with HTTP <status>." (K/ITopClient.swift:110-112, K/ITopError.swift:44-45)
- ERR-4: Body isn't a JSON object with an integer `code` → "The server didn't answer like iTop. Check that the address points to your iTop." (K/ITopClient.swift:113-117, K/ITopError.swift:50-51)
- ERR-5: iTop code 1 whose message (lowercased) contains "rest services", "rest service" or "web services" → REST-profile refusal: "This account can't use the iTop REST API. Ask your iTop admin to add the "REST Services User" profile, then sign in again." (K/ITopClient.swift:125, K/ITopClient.swift:182-185, K/ITopError.swift:34-35)
- ERR-6: Otherwise code 1 with "permission" or "not allowed" → "iTop didn't allow it: <message without a leading "Error: ", ending in punctuation>" (or "iTop didn't allow it." when empty). (K/ITopClient.swift:126, K/ITopClient.swift:187-190, K/ITopError.swift:36-41)
- ERR-7: Otherwise code 1 → "Invalid credentials. Check your username and password." (K/ITopClient.swift:127, K/ITopError.swift:32-33)
- ERR-8: Any other non-zero code → "iTop returned error <code>: <message>" or "iTop returned error <code>." (K/ITopClient.swift:128, K/ITopError.swift:42-43)
- ERR-9: Writes also check each returned object's own `code`; non-zero → the server error with that object's message. (K/TicketWrites.swift:92-96)
- ERR-10: Writes off → "Changes to iTop are switched off in this build." (base text; the UI adds context, DBG-6). (K/ITopError.swift:52-53)
- ERR-11: Any other non-ITopError at sign-in shows as the "didn't answer like iTop" text. (W/LoginView.swift:125-127)
- ERR-12: Keychain failures surface through restore as the unreachable screen (AUTH-4); a failure saving the password at sign-in shows as an error on the form. (K/SessionService.swift:74, K/SessionService.swift:85)
- ERR-13: Messages that embed a reason add a final "." when the reason doesn't end in . ! or ? (composer, actions, timer). (W/TicketListModel.swift:333-335, W/TicketDetailModel.swift:64-66, W/TimerModel.swift:118-119)
- ERR-14: Action error mapping (assign, lifecycle, create): already taken "<name> took <ref> a moment ago, so it stays with them."; no team (ASG-5); cannot assign "<Status> tickets can't be assigned."; invalid choice "Pick a team and an agent first."; missing field "Fill in the <field> first." (field names: reason, service, subcategory, resolution code, solution, organization, caller, title, description); not allowed "<Status> tickets can't do that. Refresh to see the ticket as it is now."; writes off (DBG-6); else "iTop didn't <verb>. <reason>". (W/TicketListModel.swift:316-337)

### Offline / stale
- ERR-15: There is no offline mode, outbox or offline banner: when iTop can't be reached, reads show the error states below and writes fail with their error; the last loaded list stays on screen with "Couldn't refresh: …". The timer keeps running locally. (W/TicketListModel.swift:161-163)
- ERR-16: There's no "ticket changed in iTop" check before sending; entries are sent as they are. (W/TicketDetailModel.swift:47-56)

### Empty / loading copy (index)
- ERR-17: Lists: LIST-44, LIST-60-63. Detail: DET-4-5, DET-31, DET-36. Menu bar: MB-14. CMDB: CMDB-1, CMDB-4, CMDB-10, CMDB-14-15. Orgs: ORG-4, ORG-10. People: PPL-1, PPL-4, PPL-13. Placeholders: SHELL-7. Dropdowns: ASG-9-10, LC-17, NEW-6-8. (see each item)

---

## 19. Polling, refresh intervals and timings

- POLL-1: My tickets loads as soon as the session starts (if not loaded yet) and then every **120 s**, whether or not the main window is open, because it drives the menu bar. (W/Workspace.swift:74-81)
- POLL-2: Team tickets loads when its view is first shown and then every **120 s only while the Team tickets view is shown**; leaving the section stops the loop. (W/TicketListView.swift:35-44)
- POLL-3: Each list view also loads on appear if it hasn't loaded successfully yet. (W/TicketListView.swift:36)
- POLL-4: Organizations, People, CMDB and their details never poll; they load on selection or search (org list once per session). (W/OrgsModel.swift:67-71)
- POLL-5: The ticket detail reloads when the list's copy changes status, agent, team or last update (so after each 120 s reload that changed it). (W/TicketDetailView.swift:63)
- POLL-6: Time totals reload after every list reload. (W/TicketListModel.swift:156-160)
- POLL-7: Debounces: full-ref lookup 250 ms; CMDB and People search 300 ms. (W/TicketListModel.swift:193, W/CMDBModel.swift:88, W/OrgsModel.swift:154)
- POLL-8: UI clocks: list footer "Updated …" every 30 s; live counters (header, rows, menu bar rows) every 1 s; the pending/resolve sheet's timer line every 30 s; the menu bar label every 1 s with a timer running, else every 15 s. (W/TicketListView.swift:260, W/TicketListView.swift:321, W/LifecycleSheets.swift:23, W/Workspace.swift:64-73)
- POLL-9: "Copied …" chips last 2 s. Menu bar slide: 12 steps × 18 ms. (W/TicketDetailModel.swift:98-103, W/CMDBModel.swift:118-124, W/MenuBarView.swift:30-33)
- POLL-10: All requests in a list load (two classes) run in parallel; org/person/CI ticket queries run one class after the other. (K/TicketQueries.swift:106-116, K/Organization.swift:187-194)

---

## 20. Theme and visual tokens (for exact look)

- THEME-1: Colours (light / dark): text #1D1D1F / #F2F2F7; secondary #5E5E63 / #A1A1A6; separator #D8D8DE / #3A3A3C; warning #C4291C / #FF6B5E; primary fill #1D1D1F / #F2F2F7; on-primary #FFFFFF / #1D1D1F; chip #EEEEF1 / #333336; accent (needs you only) #0F6E56 / #5DCAA5; accent tint #E1F5EE / #085041; accent text #0F6E56 / #9FE1CB; on-accent #FFFFFF / #1D1D1F. Follows the system appearance. (W/Theme.swift:7-31)
- THEME-2: Teal is used only for "needs you": the row dot, the "Needs you" pill, the sidebar count, the primary button on a needs-you ticket. Filter buttons, the timer and switches are never teal. (W/Theme.swift:15, W/ListOptions.swift:136-153)
- THEME-3: Refs and durations are monospace everywhere (rows 11.5 pt, header 12 pt, menu bar 11 pt). (W/TicketListView.swift:303-305, W/TicketDetailView.swift:87)
- THEME-4: Header controls are 26 pt high, 6 pt radius; the primary button's disabled state is 50 % opacity; the full-width filled button (sign in, sheet Assign to me) is ≥ 36 pt, 7 pt radius. (W/Theme.swift:35-81)

---

## 21. Described in the docs but NOT implemented (do not add)

These appear in SPEC.md / UX-FLOWS.md / DESIGN.md / README but the code doesn't do them. Parity means **not** building them.

- GAP-1: No macOS notifications, no first-run "turn on notifications" card, no Settings window (⌘,), no "open at login", no refresh-interval or start-view setting. (DESIGN.md "Notifications", "First run", "Settings"; no code)
- GAP-2: No "Time for this entry" field in the log composer; the composer never attaches time. (DESIGN.md "Time spent"; W/TicketDetailView.swift:383-511)
- GAP-3: No offline banner ("Offline. Showing tickets from 10:42."), no "will work again when you're back online". (UX-FLOWS decision 2)
- GAP-4: No session-expired sheet. (UX-FLOWS F8)
- GAP-5: No "ticket changed in iTop" banner, "Show changes", "Send anyway" or "New" labels on entries. (UX-FLOWS F9)
- GAP-6: No "Assign to me" in the menu bar panel. (UX-FLOWS F4, U2)
- GAP-7: No "Continue the ticket you started at …?" prompt; the draft is restored silently (NEW-22). (UX-FLOWS F7)
- GAP-8: No Team tickets team filter, no note about teal under the filters. (DESIGN.md "Team tickets")
- GAP-9: Caller and organization in the detail strip aren't links. (UX-FLOWS F6)
- GAP-10: No ⌘C on a selected row to copy its ref; no explicit ⌘F binding (only whatever the system search field gives). (DESIGN.md "Keyboard")
- GAP-11: Escape on a sheet with typed text doesn't ask before closing. (UX-FLOWS U3)
- GAP-12: List rows show the agent's name (or "You"), not initials; the org/person tables use initials + name. (DESIGN.md "Assignee" vs W/TicketListView.swift:376-380)
- GAP-13: No "Show all 212" link per status in org views; paging (ORG-11) replaces it. Digits-only ref search doesn't look up. (UX-FLOWS F5, F6)
- GAP-14: The timer doesn't stop because a colleague reassigned the ticket unless My tickets reloads and it's gone or no longer assigned to you (TMR-14); there's no "reassigned to Agent 2" wording. (UX-FLOWS F3)

---

## 22. Things that depend on macOS-native APIs

| API | Used for | Where |
|---|---|---|
| Keychain (Security framework, generic password) | Storing, reading, deleting the iTop password per "user@server"; debug service name per settings suite | K/KeychainStore.swift, K/SessionService.swift:61-94 |
| UserDefaults / `@AppStorage` (+ named suites) | Server, username, View options, running timer, New ticket "Assign to me"; debug suites `ITopWrap.debug.<name>` | K/SessionService.swift:4-21, W/ListOptions.swift, K/TimeTracking.swift:92-113, W/NewTicketView.swift:44 |
| `MenuBarExtra` (window style) + `isInserted` | Menu bar item present only while signed in; panel UI | W/App.swift:73-90 |
| NSImage template drawing, `NSFont.menuBarFont`, monospaced digits | The stacked-stubs icon drawn in code and the text drawn into the same template image so digits don't shift | W/MenuBarIcon.swift |
| `NSWorkspace.accessibilityDisplayShouldReduceMotion` | Skips the menu bar slide | W/MenuBarView.swift:25 |
| NSApplication activation policy, `activate()`, `applicationShouldTerminateAfterLastWindowClosed` | Regular Dock app; stays alive with no windows while signed in; quits when signed out | W/App.swift:160-173 |
| SwiftUI `Window` scenes, `openWindow`, `.commands`, `commandsRemoved`, `restorationBehavior`, `defaultLaunchBehavior`, `windowResizability(.contentSize)` | Single main window, separate New Ticket window, File/View menu items and shortcuts, window sizes following content | W/App.swift:14-138 |
| `NavigationSplitView`, `List` selection, `.searchable(placement: .toolbar)`, toolbar items | Three-column shell, sidebar, toolbar search and Refresh | W/ShellView.swift, W/TicketListView.swift |
| Sheets, `confirmationDialog`, segmented `Picker`, inline menu pickers, `Menu` | Assign/Pending/Resolve/New ticket/image viewer sheets; sign-out and close confirmations; View and org filter menus; log/People tabs | W/LifecycleSheets.swift, W/ShellView.swift:95-99, W/TicketDetailView.swift:166-180, W/ListOptions.swift:78-119 |
| `NSPasteboard.general` | Copy ref, copy CI name; reading pasted images (file URLs, PNG, TIFF) | W/TicketDetailView.swift:81-88, W/CMDBView.swift:138-145, W/ComposerTextView.swift:57-83 |
| `NSTextView` subclass in `NSScrollView` | Composer: paste override, Edit ▸ Paste validation, drag-and-drop of files/PNG/TIFF, Tab focus moves, focus/drop callbacks | W/ComposerTextView.swift:89-219 |
| `NSBitmapImageRep`, `NSGraphicsContext`, ImageIO (`CGImageSource`) | Converting TIFF→PNG and other images→JPEG 0.85, downscaling to 2560 px; reading pixel size for `<img width/height>` | W/ComposerTextView.swift:31-53, K/LogImage.swift:49-55 |
| `fileImporter` (open panel, security-scoped URLs) | Paperclip image picking | W/TicketDetailView.swift:498-509 |
| `NSSavePanel` | Saving a downloaded attachment | W/RichTextView.swift:184-188 |
| `NSWorkspace.open`, SwiftUI `Link` / attributed-string links | Open in iTop on the web, links in entries, outside image chips, mailto | W/CMDBView.swift:148, W/TicketDetailView.swift:142-144, W/RichTextView.swift:51-60, W/OrgsView.swift:313-315 |
| `NSImage` decoding, `NSCache` | Showing inline images and draft thumbnails | W/RichTextView.swift:115-121, W/TicketDetailView.swift:519-526 |
| `NSColor` dynamic provider / `NSAppearance` | Light/dark colour tokens; debug forced appearance | W/Theme.swift:25-31, W/App.swift:572-580 |
| URLSession with system TLS trust | All REST calls; certificate failures mapped, never bypassed | K/ITopClient.swift:100-108, K/ITopClient.swift:192-200 |
| `TimelineView`, `Task.sleep` loops | Live counters, footer "Updated …", polling loops | W/TicketListView.swift:260, W/Workspace.swift:62-82 |
| Accessibility (labels, hints, `AccessibilityNotification.Announcement`, focus/`onKeyPress`) | VoiceOver names on icon-only controls, dropdown announcements, keyboard handling | W/DropdownField.swift, throughout |
| Date/locale formatting (`DateFormatter` templates, `formatted(.relative)`, `ByteCountFormatter`, `ListFormatter`) | Log dates, "Updated 2 minutes ago" in the panel, file sizes, "A, B and C" in CI sentences | K/CaseLog.swift:98-119, W/MenuBarView.swift:218, W/TicketDetailView.swift:528-530, W/CMDBView.swift:183 |
| `freopen(stderr)` | Debug: copy warnings to a file | W/App.swift:566-570 |
