# iTop Wrap component kit

Presentational, data-agnostic React components for the screens. Props in, callbacks out: no backend
calls, no app state. Import from `@/components/kit`. See every component in every state at
`/kit.html` (`npm run dev`, then open `http://localhost:1420/kit.html`; `?theme=dark`,
`?only=<section>` and `?open=dropdown|search|menu|resolve|close` help with screenshots).

Built from the Beautiful UI sources in `ui/vendor/beautiful-ui/` (MIT, (c) Shane Levine). "Adapted"
means the markup, classes and motion were kept and the demo data, scripted animation and unused
variants removed. "Hand-built" means no source fits, so it was made in the same visual language.

## Meaning rules (DESIGN.md) the kit enforces

- **Red `need-*` means "this ticket needs you" (Escalated TTR / TTO), and nothing else.** (Was teal.) Tokens `need`, `need-tint`, `need-ink` and
  `on-need` are used only by `NeedsYouPill`, `NeedsYouDot`, `CountBadge tone="need"` and
  `Button variant="need"` (no longer used by the app). `danger` red is for priority 1, errors and Resolve and Close. Filters, timers,
  checkboxes and selection are neutral. Beautiful UI's blue `accent` is used only for the focus ring.
- **Status is always text** (`StatusPill`), on a neutral tint.
- **Red (`danger`) is used only for priority 1 (`PriorityMark`) and for errors** (field errors,
  `NoticeBanner tone="error"`, `ErrorState`, the composer error line). `Button` has no red variant.
- **Refs and durations** use `Ref`, `Duration`, `RefChip` and the `.mono-figures` class (JetBrains
  Mono with tabular figures).
- **A running timer** is shown by the icon changing from stopwatch to stop, a live counter and bold
  figures. Its colour never changes.
- **Light and dark**: tokens switch on `.dark`. Call `useSystemTheme()` from `@/hooks/useSystemTheme`
  once per window.
- **Motion** covers hover, press, menus and sheets only. `prefers-reduced-motion` stops everything
  (global rule in tokens.css). The pixel loader is the only animation that loops, and only while
  something is loading.
- **Keyboard**: every control can be reached by keyboard and shows a visible focus ring. The roles
  are listbox/option, combobox, dialog/alertdialog, menu/menuitem*, tablist/tab, radiogroup and
  separator.

## Components

| Component | File | Props (summary) | Source |
|---|---|---|---|
| `AppShell` | AppShell.tsx | `sidebar`, `children` | Adapted: Sidebar Nav (layout) |
| `Sidebar` | AppShell.tsx | `sections: {id,title?,items:{id,label,icon,count?,countTone?:'need'\|'neutral',countLabel?,shortcut?}[]}[]`, `selectedId`, `onSelect`, `user?: {name,detail?,initials?,menu: MenuEntry[],ariaLabel?,tooltip?}`, `header?`, `width=216` | Adapted: Sidebar Nav. Kept the rows, GlideMenu hover, selected tint and footer. Removed the workspace switcher, chat search, recents, collapse and Central Icons. Added a 52px `data-tauri-drag-region` that leaves 78px for the traffic lights, count badges, ⌘ hints, ↑/↓ navigation and a user menu. |
| `MasterDetail` | Layout.tsx | `list`, `detail`, `defaultListWidth=360`, `min=300`, `max=460`, `listWidth?`/`onListWidthChange?`, `listLabel` | Hand-built. The divider is a `role=separator`; drag it or use ←/→. |
| `PaneHeader` | Layout.tsx | `title?`, `subtitle?`, `leading?`, `actions?`, `children?` (rows under the title) | Hand-built. 52px drag region. |
| `Listbox` | Listbox.tsx | `label`, `selectedId`, `onSelect(id)`, `onOpen?(id)`, `onKeyDown?` | Hand-built. One tab stop, ↑/↓/Home/End move the selection, ↩ opens, uses aria-activedescendant. |
| `ListSection`, `SectionHeader` | Listbox.tsx | `title`, `count?` (sticky group header, whole-group count) | Adapted: Sidebar Nav section label |
| `useListboxOption(id)` | Listbox.tsx | hook that returns `{selected, props}` for custom rows | Hand-built |
| `TicketRow` | TicketRow.tsx | `id`, `ticketRef`, `title`, `priority?`, `needsYou?`, `age?`, `ageTitle?`, `caller?`, `org?`, `status?`, `total?`, `runningSince?` (epoch ms), `runningLabel?` (Rust's live `h:mm`; wins over runningSince), `assignee?: ReactNode`, `actions?: ReactNode` (shown on hover/focus) | Adapted: Task Rows (List variant), laid out as 3 lines inside a Sidebar Nav row |
| `SimpleRow` | TicketRow.tsx | `id`, `title`, `subtitle?`, `leading?`, `trailing?`, `mono?` (CMDB results, orgs, people) | Adapted: Task Rows |
| `TicketTable` | TicketTable.tsx | `columns?: ('ref'\|'title'\|'caller'\|'assignee')[]`, `groups: {title,count,rows:{id,ticketRef,title,priority?,needsYou?,caller?,assigneeName?,unassigned?}[]}[]`, `onOpen(id)` | Adapted: Filter Table |
| `Pager` | Pager.tsx | `first`, `last`, `total`, `onPrevious`, `onNext`, `noun?`. Shows "21–40 of 556", or "1 ticket" / "n tickets" when there is one page | Adapted: Records Table footer |
| `ListFooter` | Pager.tsx | `pager?`, `refreshing?`, `updatedLabel?` | Adapted: Records Table footer |
| `ViewSummary` | Pager.tsx | `children` (quiet 2-line summary of a non-default view) | Hand-built |
| `SearchField` | SearchField.tsx | `value`, `onChange`, `placeholder?`, `label?`, `busy?`, `trailing?`, `onKeyDown?`. Esc clears | Adapted: Search (input row) |
| `Menu` | Menu.tsx | `label`, `items: MenuEntry[]` (item / checkbox / radio / separator / section, with `icon`, `shortcut`, `disabled`, `hint`), `trigger(props)`, `placement?`, `minWidth?`, `footer?` | Adapted: GlideMenu + Records Table ConfigPicker/InputPicker; uses role=menu with full keyboard support |
| `MoreMenu` | Menu.tsx | `items`, `label="More actions"`, `variant?`, `size?` (the "…" menu) | Adapted (as above) |
| `FilterChip` | Menu.tsx | `icon?`, `label`, `active?` (dark border), `tooltip?`, `maxWidth=200`, plus the Menu trigger props | Adapted: Filter Table chips |
| `DropdownField` | DropdownField.tsx | `items: {value,label}[]`, `value`, `onChange`, `placeholder?`, `searchable?`, `searchPrompt?`, `noMatchText?`, `loading?`, `disabled?`, `invalid?`, aria props | Adapted: Search results card + Fine-tune Card select. Behaviour follows DropdownField.swift (KEY-11…17): one open at a time, flips above when there is no room below, typeahead, Esc stays inside the field. |
| `Select` | DropdownField.tsx | same as `DropdownField` without search | same component |
| `Popover` | Popover.tsx | `anchorRef`, `open`, `onDismiss?`, `placement?`, `matchWidth?`, `maxHeight?` (also `useAnchoredPosition`, `useDismiss`) | Adapted: Beautiful UI menu card, rendered in a portal |
| `Dialog` | Dialog.tsx | `open`, `title`, `subtitle?`, `children`, `onCancel`, `onConfirm?`, `confirmLabel`, `cancelLabel`, `confirmVariant?`, `confirmDisabled?`, `confirmBusy?`, `confirmTooltip?`, `error?`, `footerStart?`, `footer?`, `width=420`, `role?`, `initialFocusRef?` | Hand-built on the window surface, using Approval Card's layout. Esc cancels, ⌘↩ confirms, focus is trapped and restored. Clicking the backdrop does not close it. |
| `Field` | Form.tsx | `label`, `id?`, `required?`, `requiredError?`, `hint?` (ⓘ), `error?`, `reserveErrorSpace=true`, `children` (nodes, or a function of `{id, describedBy, invalid, required}`) | Hand-built |
| `TextInput`, `TextArea` | Form.tsx | native props + `invalid?` (`minHeight?` for TextArea) | Adapted: Chat composer field |
| `Checkbox` | Form.tsx | `checked`, `onChange`, `label`, `disabled?` (neutral ink fill, never accent) | Adapted: Records Table InputPicker check |
| `SegmentedControl` | Form.tsx | `options: {value,label}[]`, `value`, `onChange`, `label`, `size?` (radiogroup, ←/→) | Adapted: Fine-tune Card segmented |
| `InfoHint` | Form.tsx | `text` (ⓘ button; tooltip and accessible name) | Hand-built |
| `Tabs`, `TabPanel` | Tabs.tsx | `id`, `items: {value,label,count?}[]`, `value`, `onChange`, `label` / `tabsId`, `value` | Adapted: Chat header tabs |
| `StatusPill` | Badges.tsx | `children` | Adapted: Filter Table pill / ValuePill (neutral) |
| `NeedsYouPill`, `NeedsYouDot` | Badges.tsx | `label?` / `size=8` | Hand-built (red) |
| `PriorityMark` | Badges.tsx | `priority` (P1 red "!", P2 grey dot, P3/4 nothing) | Hand-built |
| `CountBadge` | Badges.tsx | `count`, `tone?`, `label?` | Adapted: Sidebar Nav count |
| `Ref`, `Duration` | Ref.tsx | `children`; Duration `running?` (bold) | Hand-built (the mono motif) |
| `RefChip` | Ref.tsx | `value`, `onCopy?` (defaults to navigator.clipboard), `copiedMs=2000`, `size?` | Adapted: Code Block copy button |
| `CopiedChip` | Ref.tsx | `children` ("✓ Copied R-000412") | Hand-built |
| `InitialsAvatar`, `UnassignedMark`, `Assignee`, `AssignToMePill`, `initialsOf` | Avatar.tsx | `name`, `initials?`, `size?` / `name?`, `unassigned?`, `compact?` / `ticketRef`, `onClick` | Adapted: EntityChip / Monogram (neutral disc, no colours) |
| `SectionHeading`, `KeyValueGrid`, `DetailStrip`, `DetailItem`, `ClassIcon` | Detail.tsx | `items: {key,label,value}[]`, `minColumn=160` / `icon`, `label`, `children`, `emptyText` / `kind` | Hand-built |
| `LogEntry` | Detail.tsx | `author?`, `date?`, `isPrivate?` (lock icon labelled "Private"), `children` (rich content) | Adapted: Beautiful UI card (shadow-card) |
| `AttachmentChip` | Detail.tsx | `name`, `onClick?`, `busy?`, `disabled?` | Adapted: Context Cards chips |
| `CIChip` | Detail.tsx | `name`, `tooltip?`, `onClick?`, `variant: 'chip'\|'link'` | Adapted: Context Cards chips |
| `Composer` | Composer.tsx | `value`, `onChange`, `onSend`, `label`, `placeholder?`, `sendLabel`, `sendDisabled?`, `sendTooltip?`, `busy?`, `readOnly?`, `images?: {id,src,name,sizeLabel?}[]`, `onRemoveImage?`, `onAttach?`, `attachDisabled?`, `attachTooltip?`, `status?`, `statusTone?`, `onPaste?(event)`, `onDropFiles?(files, event)` | Adapted: Prompt Bar (Rounded) + Chat composer. Removed the demo, slash menu, model picker, dictation and glimm. Added a 64px multi-line editor, ⌘↩ to send, thumbnails and a status line. |
| `TimerButton` | TimerButton.tsx | `state: 'idle'\|'running'\|'web'`, `startedAt?`, `webSince?`, `onToggle?`, `disabled?`, `busy?`, `tooltip?` | Hand-built in the header-control style |
| `Button` | Button.tsx | `variant: primary\|secondary\|ghost\|quiet\|need`, `size: xs\|sm\|md\|block`, `icon?`, `busy?`, `tooltip?` (also `kitButtonClass`) | Adapted: atoms/Button (accent and success removed, `need` and `block` added) |
| `IconButton` | IconButton.tsx | `label` (accessible name, and the tooltip unless one is given), `icon`, `tooltip?`, `size: xs\|sm\|md`, `variant: quiet\|field`, `busy?`, `children?` | Adapted: `.primitive-icon-button` |
| `Tooltip` | Tooltip.tsx | `content`, `side?`, `describe?`. Delayed on hover, immediate on keyboard focus, also works on disabled buttons | Adapted: Beautiful UI tooltip tokens |
| `Spinner` | Spinner.tsx | `size?`, `label?` | Adapted: Task Rows SpinnerRing |
| `LoadingState`, `LoaderGrid` | States.tsx | `label` | Adapted: Loading State (the "Drive" grid only; no shimmer, no elapsed timer, no Surfer video) |
| `EmptyState` | States.tsx | `title`, `hint?`, `icon?`, `action?` | Adapted: Search empty state |
| `ErrorState` | States.tsx | `message`, `onRetry?`, `retryLabel?`, `retrying?` | Adapted: Search empty state |
| `NoticeBanner` | States.tsx | `tone: info\|error`, `icon?`, `children`, `onRetry?`, `retryDisabled?`, `onDismiss?`, `dismissLabel?` | Hand-built |
| `Toast`, `ToastRegion`, `useToasts` | Toast.tsx | `toasts`, `show(content, ms=2000)` | Hand-built |
| `MenuBarPanel`, `MenuBarTopRow`, `MenuBarSection`, `MenuBarTicketRow`, `MenuBarFooter` | MenuBar.tsx | panel at 360px / `query`, `onQueryChange`, `onNewTicket` / `title`, `count?` / `ticketRef`, `title`, `needsYou?`, `total?`, `runningSince?`, `runningLabel?` (Rust's live `HH:MM:SS`), `timer: start\|stop\|none`, `onToggleTimer`, `timerTooltip?`, `onOpen` / `onRefresh`, `refreshing?`, `updatedLabel?`, `onOpenApp` | Adapted: Task Rows (List) + GlideMenu + Search |
| icons | icons.tsx | iconoir re-exports under meaning names (`StopwatchIcon`, `TrayIcon`, …), plus `StopIcon` and `ChevronUpDownIcon` | iconoir-react (the paid Central Icons are not used) |
| `formatCounter`, `formatClock`, `useNow`, `useElapsedSeconds` | time.ts | display helpers for live counters only (rules stay in Rust) | Hand-built |
| `scrollIntoNearest` | scroll.ts | scrolls inside the nearest scroll container only | Hand-built |

## Global CSS changes (src/styles/app.css)

- Fonts: @fontsource registers the faces as "Inter Variable" and "JetBrains Mono Variable", so the
  font variables are pointed at those names.
- The site's striped body background is gone. The app backdrop is `--page` and body text is 13px.
- The focus ring rules in tokens.css are unlayered. That made a focused pill button square and
  stopped components from opting out. app.css hands them back with `revert-layer` and sets the
  same ring again in `@layer base`.
- App tokens: `need-*`, `danger*` (darker than `--red`, so small text passes AA), `--app-bg`,
  `--sidebar-bg` and `--focus`.
- tokens.css is untouched.

## Tests

`kit.test.tsx` covers:

- DropdownField keyboard behaviour: open on the selection, move, pick, typeahead, Esc layering,
  filtering, the no-match text, and that an empty list can't open.
- Dialog: role, initial focus, Tab trap, Esc and ⌘↩, disabled confirm, and Esc with a dropdown open.
- Composer: ⌘↩ sends while Return adds a line, a disabled send does nothing, and dropped files
  reach the caller.
- RefChip: copies, shows "Copied" for 2 s, and stays quiet when the copy fails.
