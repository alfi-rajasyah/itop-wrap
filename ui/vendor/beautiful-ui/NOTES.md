# Beautiful UI: recovered sources

Source: https://www.beautifului.dev (MIT, (c) Shane Levine). Keep the MIT notice when vendoring.

## Where the code came from

- **Saved HTML**: all 21 component sources are RSC text rows (`<id>:T<len>,...`) in the flight payload. A `sources` map in the payload ties each slug to its row. Each one was extracted byte for byte.
- **Shadcn registry** (`https://www.beautifului.dev/r/registry.json`, using the `ui.shadcn.com` schema): 27 items. That is 1 style (`foundation`), 6 shared atoms and 20 components. Every registry `.tsx` is **identical** to its HTML extract. The registry also holds files the HTML does not have: the atoms, `foundation.css`, `records-table.css` and `sidebar-nav.css`.
- **Agent Screen** is not in the registry (`/r/agent-screen.json` returns 404). Its source comes only from the HTML, but that copy is complete.
- The page gives no install or usage text. The registry implies the install is `npx shadcn@latest add https://www.beautifului.dev/r/<name>.json`, which pulls in `registryDependencies` (foundation and atoms) automatically. The site does not state this. The foundation item's description says: add `@import "./beautifui/foundation.css";` to your app's globals.css.
- Every file is saved verbatim. Only `tokens.css` and `lib-utils.ts` were written by me, and each says so in a header.

## Files

| File | Original path (registry target) |
|---|---|
| `<slug>.tsx` (21) | `components/primitives/<Pascal>.tsx` (search.tsx is `SearchList.tsx`) |
| `atoms/button.tsx`, `glide-menu.tsx`, `entity-chip.tsx`, `value-pill.tsx`, `shimmer.tsx`, `stream-text.tsx` | `components/atoms/Button.tsx` and siblings; GlideMenu is at `components/primitives/GlideMenu.tsx` |
| `foundation.css` | `app/beautifui/foundation.css` (verbatim) |
| `records-table.css`, `sidebar-nav.css` | `app/beautifui/*.css` (verbatim, component-specific) |
| `tokens.css` | Consolidated theme: foundation.css with 3 marked changes (see the header) |
| `lib-utils.ts` | **Not from the site**: the standard shadcn `cn()` that Button imports from `@/lib/utils` |

The components import `@/components/atoms/Button`, `@/components/atoms/EntityChip`, `@/components/atoms/ValuePill`, `@/components/atoms/Shimmer`, `@/components/atoms/StreamText`, `@/components/primitives/GlideMenu` and `@/lib/utils`. You must either restore that layout under an `@/` alias or rewrite those imports. In this folder the atoms are saved under kebab-case names.

## Tailwind, theme, fonts

- **Tailwind v4** (the compiled CSS banner says `tailwindcss v4.3.3`). The theme uses `@import "tailwindcss"`, `@theme inline { --color-* ... }` and `@custom-variant dark (&:where(.dark, .dark *))`. There is no v3 config.
- **Dark mode** works through a `.dark` class on an ancestor. Tokens are redefined under `.dark`. It does not follow `prefers-color-scheme` on its own.
- **Tokens**:
  - Surfaces: page, canvas, surface, inset, hover, hover-2, field.
  - Ink: ink, ink-2, ink-3.
  - Lines: line, line-strong, line-soft, grid-line.
  - Accent: accent, accent-ink, accent-tint.
  - Status: green, orange and red, each with a `-tint`.
  - Tooltip: tooltip-*.
  - Shadows: hairline, btn, card, raised, overlay, inset-field.
  - Radii: chip 6, control 8, card 10, window 14.
  - Easings: out-strong, in-out-strong, link.
  - Utilities: `.primitive-card-pad`, `-bar`, `-footer`, `-table-cell`, `-icon-button`.
  - Keyframes: shimmer-text, fade-up, fade-in, eq-bounce, caret-blink, pop-in, spin, pixel-on.
  - Component blocks: source-avatar, filter-status-*, insight-chart-*, stream-tail/stream-caret.
- **Shadows**: foundation.css imports the npm package `shadow-plugin/unprefixed`, which supplies `--shadow-xs..lg`. In tokens.css I replaced that import with the values from the compiled site CSS.
- **Bug in the registry foundation.css**: around line 422 there is an orphaned `.records-footer-hint{display:none}` followed by a stray `}`. Its `@media (max-width:640px)` opener is missing. This is fixed in tokens.css.
- **Body base**: the base styles put a diagonal stripe on `body` (`--stripe` with `background-attachment: fixed`). They also set the body to 14px with `letter-spacing:-0.01em` and add `font-feature-settings "cv11","ss01"`. These are global, so strip them if they clash with the host app.
- **Global reduced-motion rule**: `@media (prefers-reduced-motion)` clamps every animation and transition on the page.
- **Fonts**: **Inter** (variable 100–900) for sans and **JetBrains Mono** (variable 100–800) for mono. The site loads both through next/font as `--font-inter` and `--font-mono-face`. tokens.css defines those variables, but you have to load the fonts yourself (for example `@fontsource-variable/inter` and `@fontsource-variable/jetbrains-mono`).

## npm dependencies

- Always: `react`, `react-dom`, `tailwindcss@4`.
- `class-variance-authority`, `clsx`, `tailwind-merge`: for Button and `cn`. Button is used by approval-card, diff-table, recommendation-card, selection-actions and agent-screen.
- `@central-icons-react/round-outlined-radius-2-stroke-2`: sidebar-nav only, 12 icons. Central Icons is a commercial icon set, so check the license and registry access before relying on it.
- `iconoir-react`: selection-actions.
- `glimm`: prompt-bar (WebGL shader sweep).
- `liveline`: insight-cards (charts).
- `shadow-plugin`: optional if you use tokens.css.
- No motion/framer-motion, radix, lucide or clsx in the components themselves. All animation is CSS plus requestAnimationFrame and timers.

## Components

Legend: **Complete** means the full source was recovered and matches the registry.

Every component accepts `variant?: string`. Most also take a `labels?: Partial<...Labels>` object for copy, and default data arrays, so they render as self-running demos with no props.

Dark mode: every component is styled with token classes (`bg-surface`, `text-ink`, ...), so it follows `.dark`. The few hard-coded colours are brand logos, chart series or status hues.

| # | Component | File | Status | Props API | Deps |
|---|---|---|---|---|---|
| 1 | Loading State | loading-state.tsx | Complete | `label?`, `variant?` = Drive/Dots/Orbit/…/Surfer, `videoSrc?`. **Surfer streams an MP4 from a Vercel Blob URL by default.** | none |
| 2 | Thinking | thinking-state.tsx | Complete | `variant?` = Steps/Reasoning/Search/Coding, `rows?`, `active?`, `done?`, `icon?`, `onSettled?` | none |
| 3 | Streaming Text | streaming-text.tsx | Complete | `content?: StreamingToken[]`, `sources?`, `followUps?`, `labels?`, `loop?`, `fill?`, `onDone?`, `onFollowUp?` | foundation `.source-avatar` |
| 4 | Approval Card | approval-card.tsx | Complete | `questions?: ApprovalQuestion[]`, `labels?`, `onSubmitted?(answers)`, `onAnswerChange?`, `resettable?` | Button, GlideMenu |
| 5 | Tool Chips | tool-chips.tsx | Complete | `steps?: ToolStep[]`, `diffs?`, `diffLines?`, `labels?`, `className?`, `onOpenChange?`, `onToggleRow?` | react-dom portal |
| 6 | Task Rows | task-rows.tsx | Complete | `variant?` = Capsules/List, `rows?: TaskRow[]` (`{key,label,amount,status:"done"\|"running"\|"sequence",step?,details[]}`), `labels?`, `className?`, `onToggleRow?` | none |
| 7 | Chat | chat-composer.tsx | Complete | `messages?: ChatMessage[]` (scripted replies), `suggestions?: string[]` (tabs), `labels?`, `onSend?(text)` | none |
| 8 | Prompt Bar | prompt-bar.tsx | Complete | `variant?` = Rounded/Pill, `demo?` (self-running walkthrough, **set false in a real app**), `tall?`, `placeholder?`, `onSend?` | glimm |
| 9 | Recommendation Card | recommendation-card.tsx | Complete | `options?: RecommendationOption[]`, `labels?` | Button, EntityChip, ValuePill |
| 10 | Context Cards | context-cards.tsx | Complete | `chunks?: ContextChunk[]`, `labels?`, `className?` | foundation `.source-avatar` |
| 11 | Diff Table | diff-table.tsx | Complete | `rows?: DiffRow[]` | Button |
| 12 | Records Table | records-table.tsx + records-table.css | Complete | `rows?: RecordRow[]` (`{id,name,tags[],last,strength,website?}`), `fill?`. Domain-specific CRM grid (about 1,500 lines with the AI column) | GlideMenu, records-table.css |
| 13 | Filter Table | filter-table.tsx | Complete | `rows?: TableRow[]` (`{task,date,status:"todo"\|"progress"\|"done",owner}`), `labels?: {columns}` | foundation `.filter-status-*` |
| 14 | Sidebar Nav | sidebar-nav.tsx + sidebar-nav.css | Complete | `activeTitle?`, `activeNav?`, `onNavigate?(key)`, `onNewChat?`, `onPick?(id,label,prompt?)`, `recents?: {id,label,prompt?}[]`, `footerLabel?`, `footerIcon?`, `onFooterClick?`, `fill?`, `className?`. Collapses from 224px to 52px; uses a portal menu | central-icons, GlideMenu, react-dom |
| 15 | Search | search.tsx (`SearchList`) | Complete | `items?: string[]`, `labels?: {placeholder,ariaLabel,emptyTitle,emptyHint}` | GlideMenu |
| 16 | Flowchart | flowchart.tsx | Complete | `steps?: StepNode[]` | none |
| 17 | Insight Cards | insight-cards.tsx | Complete | `pages?: InsightPage[]`, `labels?`. Watches `document.documentElement.classList` for `dark` | liveline, foundation `.insight-chart-*` |
| 18 | Code Block | code-block.tsx | Complete | `variant?` = Code/Diff, `lines?: string[]` or `code?: string`, `diff?: DiffRow[]`, `filename?`, `labels?: {copy,copied}`, `onCopy?`. Uses `navigator.clipboard`. No syntax highlighter (hand-tokenised) | none |
| 19 | Fine-tune Card | fine-tune-card.tsx | Complete | `fields?: FineTuneField[]`, `options?`, `labels?`, `onChange?(state)` | GlideMenu |
| 20 | Selection Actions | selection-actions.tsx | Complete | `text?: Partial<SelectionText>`, `actions?: {primary: SelectionAction[], more: SelectionAction[]}` (`{id,icon,action?,busyLabel?}`), `labels?`, `onAction?(action)` | iconoir-react, Button, Shimmer, StreamText, foundation `.stream-*` |
| 21 | Agent Screen | agent-screen.tsx | Complete (HTML only, not in registry) | `agentName?`, `streamSrc?` (defaults to `/agent-desktop.webp`, a site asset not downloaded here), `variant?` (`"Loading"`). Fullscreen portal; locks `body` overflow | Button, react-dom |

Shared atoms (all complete, from the registry):

- **Button**: `variant`: primary/secondary/ghost/accent/success/quiet; `size`: xs and others. Built with cva and `cn`.
- **GlideMenu**: `children`, `className?`, `highlightClassName?`, `rowSelector?` (default `[data-menu-row]`). A pointer-following highlight.
- **EntityChip**: also exports `Monogram`.
- **ValuePill**: `tone?`.
- **Shimmer**: `children`, `className?`.
- **StreamText**: `text`, `charsPerTick?`, `tickMs?`, `blurTail?`, `caret?`, `onProgress?`, `onDone?`.

## Vendoring gotchas

1. **Demo-first components.** Defaults are ice-cream demo data, and several components auto-play scripted timelines: PromptBar `demo`, StreamingText `loop`, Thinking, TaskRows, Flowchart and others. Pass real data and turn the demo flags off. Some, such as RecordsTable, are tightly bound to their demo data shape.
2. **`"use client"` directives** are harmless in Vite/Tauri. The `@next/next` eslint comments are inert.
3. **Path aliases**: you need `@/components/atoms/*`, `@/components/primitives/GlideMenu` and `@/lib/utils`.
4. **Global CSS side effects** in foundation.css: the body stripe, font size, cursor rules, a global `:focus-visible` outline and the reduced-motion clamp. It also contains site-only selectors (`.component-nav-scroll`, `.primitive-showcase`, `.home-reveal`, `.theme-switching`) that are safe to drop.
5. **External network**: LoadingState's Surfer variant fetches an MP4 from Vercel Blob. StreamingText and ThinkingState use demo favicon/source URLs. These matter under a Tauri CSP.
6. **Paid icon package** in SidebarNav (central-icons). Swapping it out means editing 12 imports.
7. **`glimm` and `liveline`** are niche packages. I did not check them on npm (the brief allowed only this site).
