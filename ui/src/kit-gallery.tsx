import { StrictMode, useEffect, useState, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import "./styles/app.css";
import { useSystemTheme, type ThemeMode } from "@/hooks/useSystemTheme";
import {
  AppShell,
  Assignee,
  AssignToMePill,
  AttachmentChip,
  BuildingIcon,
  Button,
  CalendarIcon,
  Checkbox,
  CIChip,
  ClassIcon,
  Composer,
  CopiedChip,
  CountBadge,
  DetailItem,
  DetailStrip,
  Dialog,
  DropdownField,
  Duration,
  EmptyState,
  ErrorState,
  ExternalIcon,
  Field,
  FilterChip,
  IconButton,
  InitialsAvatar,
  KeyValueGrid,
  ListFooter,
  Listbox,
  ListSection,
  LoadingState,
  LogEntry,
  MasterDetail,
  Menu,
  MenuBarFooter,
  MenuBarPanel,
  MenuBarSection,
  MenuBarTicketRow,
  MenuBarTopRow,
  MoreMenu,
  NeedsYouDot,
  NeedsYouPill,
  NoticeBanner,
  Pager,
  PaneHeader,
  PeopleIcon,
  PersonIcon,
  PriorityMark,
  RefChip,
  RefreshIcon,
  SearchField,
  SearchIcon,
  SectionHeading,
  SegmentedControl,
  Select,
  ServerIcon,
  Sidebar,
  SimpleRow,
  Spinner,
  StatusPill,
  StopwatchIcon,
  Tabs,
  TabPanel,
  TeamIcon,
  TextArea,
  TextInput,
  TicketRow,
  TicketTable,
  TimerButton,
  ToastRegion,
  Tooltip,
  TrayIcon,
  UnassignedMark,
  useToasts,
  ViewSummary,
  ViewIcon,
  type ComposerImage,
  type DropdownItem,
  type MenuEntry,
} from "@/components/kit";

/* ── sample data (iTop-shaped, made up) ─────────────────── */

const now = Date.now();
const RUNNING_SINCE = now - (12 * 60 + 34) * 1000;

type Sample = {
  id: string;
  ref: string;
  title: string;
  priority: number;
  needsYou?: boolean;
  age: string;
  caller: string;
  org: string;
  total?: string;
  running?: boolean;
  agent?: string;
  unassigned?: boolean;
};

const GROUPS: { title: string; count: number; rows: Sample[] }[] = [
  {
    title: "Escalated TTR",
    count: 2,
    rows: [
      { id: "UserRequest::412", ref: "R-000412", title: "VPN drops every 20 minutes for the whole finance floor", priority: 1, needsYou: true, age: "3d", caller: "Ada Lovelace", org: "Demo Org A", total: "1h 20m" },
      { id: "Incident::88", ref: "I-000088", title: "Mail relay rejects messages larger than 10 MB", priority: 2, needsYou: true, age: "6h", caller: "Grace Hopper", org: "Demo Org B", total: "25m" },
    ],
  },
  {
    title: "Escalated TTO",
    count: 1,
    rows: [{ id: "UserRequest::436", ref: "R-000436", title: "New starter needs a laptop and a badge by Monday", priority: 3, needsYou: true, age: "2h", caller: "Alan Turing", org: "Demo Org A", unassigned: true }],
  },
  {
    title: "Assigned",
    count: 23,
    rows: [
      { id: "UserRequest::398", ref: "R-000398", title: "Printer on 3rd floor prints blank pages", priority: 3, age: "1d", caller: "Katherine Johnson", org: "Demo Org C", running: true },
      { id: "UserRequest::405", ref: "R-000405", title: "Shared drive permissions for the audit team, read only until the end of the quarter", priority: 4, age: "4d", caller: "Margaret Hamilton", org: "Demo Org A", total: "2h" },
      { id: "Incident::91", ref: "I-000091", title: "Wi-Fi in meeting room Ganymede is slow", priority: 2, age: "now", caller: "Edsger Dijkstra", org: "Demo Org B", agent: "Agent Two" },
    ],
  },
];

const ORGS: DropdownItem[] = ["Demo Org A", "Demo Org B", "Demo Org C", "Facilities", "Finance and Accounting", "Human Resources", "IT Services", "Legal", "Marketing"].map((o, i) => ({
  value: String(i + 1),
  label: o,
}));
const CALLERS: DropdownItem[] = ["Ada Lovelace", "Alan Turing", "Grace Hopper", "Katherine Johnson", "Margaret Hamilton"].map((o, i) => ({ value: `p${i}`, label: o }));
const RESOLUTION: DropdownItem[] = ["Assistance", "Bug fixed", "Hardware repair", "Other", "Software patch", "System update", "Training"].map((l) => ({ value: l.toLowerCase(), label: l }));
const URGENCY: DropdownItem[] = [
  { value: "1", label: "Critical" },
  { value: "2", label: "High" },
  { value: "3", label: "Medium" },
  { value: "4", label: "Low" },
];

function swatch(fill: string, label: string) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="112" height="112"><rect width="112" height="112" fill="${fill}"/><rect x="14" y="20" width="84" height="10" rx="3" fill="#ffffff" opacity=".85"/><rect x="14" y="38" width="60" height="8" rx="3" fill="#ffffff" opacity=".6"/><text x="14" y="96" font-family="monospace" font-size="12" fill="#fff">${label}</text></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

const IMAGES: ComposerImage[] = [
  { id: "a", src: swatch("#4b5563", "PNG"), name: "Pasted image.png", sizeLabel: "412 KB" },
  { id: "b", src: swatch("#6b7280", "JPG"), name: "error-dialog.jpg", sizeLabel: "1.2 MB" },
];

/* ── gallery chrome ─────────────────────────────────────── */

/** `?only=<section id>` renders one section (handy for screenshots). */
const ONLY = new URLSearchParams(location.search).get("only");
/** `?open=dropdown|search|menu|resolve|close` opens one overlay on load (screenshots). */
const OPEN = new URLSearchParams(location.search).get("open");

function Section({ id, title, source, children }: { id: string; title: string; source: string; children: ReactNode }) {
  if (ONLY && ONLY !== id) return null;
  return (
    <section id={id} className="scroll-mt-16 border-t border-line py-8">
      <div className="mb-4 flex items-baseline gap-3">
        <h2 className="text-[16px] font-semibold text-ink">{title}</h2>
        <span className="text-[12px] text-ink-2">{source}</span>
      </div>
      <div className="flex flex-col gap-5">{children}</div>
    </section>
  );
}

function Demo({ label, children, className }: { label: string; children: ReactNode; className?: string }) {
  return (
    <div className="flex flex-col gap-2">
      <span className="text-[11.5px] font-medium text-ink-2">{label}</span>
      <div className={className?.includes("block") ? className : `flex flex-wrap items-center gap-3 ${className ?? ""}`}>{children}</div>
    </div>
  );
}

/* ── shell demo ─────────────────────────────────────────── */

function ShellDemo({ onCopy }: { onCopy: (ref: string) => void }) {
  const [section, setSection] = useState("mine");
  const [selected, setSelected] = useState<string | null>("UserRequest::412");
  const [query, setQuery] = useState("");
  const [org, setOrg] = useState<string | null>(null);
  const [sort, setSort] = useState("priority");
  const [includeClosed, setIncludeClosed] = useState(false);
  const [tab, setTab] = useState<"public" | "private">("public");
  const [draft, setDraft] = useState("");

  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const groups = GROUPS.map((g) => ({
    ...g,
    rows: g.rows.filter((r) => (!org || r.org === org) && words.every((w) => `${r.ref} ${r.title}`.toLowerCase().includes(w))),
  })).filter((g) => g.rows.length);
  const ticket = GROUPS.flatMap((g) => g.rows).find((r) => r.id === selected);

  const viewMenu: MenuEntry[] = [
    { kind: "section", id: "s1", label: "Show tickets opened" },
    { kind: "radio", id: "any", label: "Any time", checked: true },
    { kind: "radio", id: "7", label: "Last 7 days", checked: false },
    { kind: "radio", id: "30", label: "Last 30 days", checked: false },
    { kind: "checkbox", id: "closed", label: "Include resolved and closed", checked: includeClosed, onCheckedChange: setIncludeClosed },
    { kind: "separator", id: "sep1" },
    { kind: "section", id: "s2", label: "Sort inside each status group" },
    ...[
      ["priority", "Priority, then oldest"],
      ["lastUpdate", "Last update, newest first"],
      ["newest", "Opened, newest first"],
      ["oldest", "Opened, oldest first"],
    ].map(([v, l]) => ({ kind: "radio" as const, id: v!, label: l!, checked: sort === v, onSelect: () => setSort(v!) })),
    { kind: "separator", id: "sep2" },
    { kind: "section", id: "s3", label: "Per page" },
    ...["10", "20", "30", "50", "100"].map((n) => ({ kind: "radio" as const, id: `pp${n}`, label: `${n} per page`, checked: n === "20" })),
    { kind: "radio", id: "ppall", label: "All", checked: false },
    ...(sort !== "priority" || includeClosed
      ? ([
          { kind: "separator", id: "sep3" },
          { id: "reset", label: "Back to default view", onSelect: () => (setSort("priority"), setIncludeClosed(false)) },
        ] as MenuEntry[])
      : []),
  ];
  const nonDefault = [includeClosed && "All tickets, any time", sort !== "priority" && "Last update, newest first"].filter(Boolean).join(" · ");
  const orgMenu: MenuEntry[] = [
    { kind: "radio", id: "all", label: "All organizations", checked: !org, onSelect: () => setOrg(null) },
    { kind: "separator", id: "s" },
    ...["Demo Org A", "Demo Org B", "Demo Org C"].map((o) => ({ kind: "radio" as const, id: o, label: o, checked: org === o, onSelect: () => setOrg(org === o ? null : o) })),
  ];

  return (
    <div className="h-[680px] overflow-hidden rounded-[14px] shadow-overlay">
      <AppShell
        sidebar={
          <Sidebar
            selectedId={section}
            onSelect={setSection}
            sections={[
              {
                id: "tickets",
                title: "Tickets",
                items: [
                  { id: "mine", label: "My tickets", icon: <TrayIcon />, count: 3, countTone: "need", countLabel: "3 need you", shortcut: "⌘1" },
                  { id: "team", label: "Team tickets", icon: <TeamIcon />, shortcut: "⌘2" },
                ],
              },
              {
                id: "cmdb",
                title: "CMDB",
                items: [
                  { id: "ci", label: "Config items", icon: <ServerIcon />, shortcut: "⌘3" },
                  { id: "orgs", label: "Organizations", icon: <BuildingIcon />, shortcut: "⌘4" },
                  { id: "people", label: "People", icon: <PeopleIcon />, shortcut: "⌘5" },
                ],
              },
            ]}
            user={{ name: "Demo Agent", detail: "itop.example.com", menu: [{ id: "out", label: "Sign out…", onSelect: () => {} }] }}
          />
        }
      >
        <MasterDetail
          listLabel="Resize the ticket list"
          list={
            <>
              <PaneHeader
                title="My tickets"
                actions={<IconButton label="Refresh" tooltip="Refresh (⌘R)" icon={<RefreshIcon />} />}
              >
                <SearchField value={query} onChange={setQuery} placeholder="R-000412 or VPN" />
                <div className="flex items-center gap-1.5">
                  <Menu
                    label="Filter by organization"
                    items={orgMenu}
                    trigger={(p) => (
                      <FilterChip
                        {...p}
                        icon={<BuildingIcon />}
                        label={org ?? "All organizations"}
                        active={!!org}
                        tooltip={org ? `Showing tickets for ${org}` : "Filter by organization"}
                        aria-label={`Filter by organization: ${org ?? "all"}`}
                      />
                    )}
                  />
                  <Menu
                    label="View options"
                    items={viewMenu}
                    minWidth={240}
                    trigger={(p) => <FilterChip {...p} icon={<ViewIcon />} label="View" active={!!nonDefault} tooltip={nonDefault || "Open tickets, sorted by priority, 20 per page"} />}
                  />
                </div>
                <ViewSummary>{nonDefault}</ViewSummary>
              </PaneHeader>
              <div className="kit-scroll min-h-0 flex-1 overflow-y-auto">
                <div className="px-3 pb-2">
                  <NoticeBanner onDismiss={() => {}}>R-000412 is assigned to you.</NoticeBanner>
                </div>
                {groups.length ? (
                  <Listbox label="My tickets" selectedId={selected} onSelect={setSelected}>
                    {groups.map((g) => (
                      <ListSection key={g.title} title={g.title} count={g.count}>
                        {g.rows.map((r) => (
                          <TicketRow
                            key={r.id}
                            id={r.id}
                            ticketRef={r.ref}
                            title={r.title}
                            priority={r.priority}
                            needsYou={r.needsYou}
                            age={r.age}
                            ageTitle="Opened 12 Nov 2026 at 09:14"
                            caller={r.caller}
                            total={r.total}
                            runningSince={r.running ? RUNNING_SINCE : null}
                            assignee={
                              r.unassigned ? (
                                <AssignToMePill ticketRef={r.ref} onClick={() => {}} />
                              ) : r.agent ? (
                                <Assignee name={r.agent} />
                              ) : undefined
                            }
                            actions={
                              r.running ? undefined : (
                                <IconButton label={`Copy ${r.ref}`} icon={<ExternalIcon />} size="xs" onClick={() => onCopy(r.ref)} />
                              )
                            }
                          />
                        ))}
                      </ListSection>
                    ))}
                  </Listbox>
                ) : (
                  <EmptyState title={`No tickets in your list match “${query.trim()}”.`} />
                )}
              </div>
              <ListFooter pager={<Pager first={1} last={20} total={26} onPrevious={() => {}} onNext={() => {}} />} refreshing updatedLabel="Updated 2 min ago" />
            </>
          }
          detail={
            ticket ? (
              <div className="kit-scroll flex min-h-0 flex-1 flex-col overflow-y-auto">
                <div data-tauri-drag-region className="h-[52px] shrink-0" />
                <div className="flex flex-col gap-4 px-6 pb-6">
                  <div className="flex flex-wrap items-start gap-3">
                    <div className="min-w-[260px] flex-1">
                      <div className="flex items-center gap-2 text-[12px] text-ink-2">
                        <RefChip value={ticket.ref} onCopy={onCopy} />
                        <ClassIcon kind={ticket.ref.startsWith("I") ? "Incident" : "UserRequest"} />
                        <span>{ticket.running ? "Assigned" : "Escalated TTR"}</span>
                        {ticket.needsYou ? <NeedsYouPill /> : null}
                      </div>
                      <h1 className="mt-1 text-[20px] font-semibold leading-tight text-ink">{ticket.title}</h1>
                    </div>
                    <div className="flex items-center gap-1.5">
                      <TimerButton state={ticket.running ? "running" : "idle"} startedAt={RUNNING_SINCE} onToggle={() => {}} />
                      <Button variant={ticket.needsYou ? "need" : "primary"} tooltip={`Resolve ${ticket.ref}`}>
                        Resolve ticket
                      </Button>
                      <MoreMenu
                        items={[
                          { id: "reassign", label: "Reassign…" },
                          { id: "pending", label: "Set to pending…" },
                          { kind: "separator", id: "s" },
                          { id: "copy", label: `Copy ref ${ticket.ref}`, onSelect: () => onCopy(ticket.ref) },
                          { id: "web", label: "Open in iTop on the web" },
                        ]}
                      />
                    </div>
                  </div>
                  <DetailStrip>
                    <DetailItem icon={<PersonIcon />} label="Caller">{ticket.caller}</DetailItem>
                    <DetailItem icon={<BuildingIcon />} label="Organization">{ticket.org}</DetailItem>
                    <DetailItem icon={<TeamIcon />} label="Team">Service desk</DetailItem>
                    <Tooltip content="Agent" describe={false}>
                      <Assignee name="You" />
                    </Tooltip>
                    {ticket.total ? <DetailItem icon={<StopwatchIcon />} label="Time spent"><Duration>{ticket.total}</Duration> spent</DetailItem> : null}
                    <CIChip variant="link" name="vpn-gw-01" tooltip="Linked CI: open vpn-gw-01 in Config items" />
                  </DetailStrip>
                  <p className="-mt-2 text-[12px] text-ink-2">Network › VPN access</p>
                  <div className="flex flex-col gap-2">
                    <SectionHeading>Description</SectionHeading>
                    <p className="whitespace-pre-line text-[13px] leading-[1.55] text-ink">
                      {"Since Monday the VPN disconnects roughly every 20 minutes.\nReconnecting works, but open RDP sessions are lost. Affects everyone on the 4th floor."}
                    </p>
                    <SectionHeading className="mt-1">Attachments</SectionHeading>
                    <div className="flex flex-wrap gap-1.5">
                      <AttachmentChip name="vpn-client.log" />
                      <AttachmentChip name="screenshot 2026-11-12.png" busy />
                    </div>
                  </div>
                  <div className="flex items-center justify-between">
                    <Tabs
                      id="log"
                      label="Log"
                      value={tab}
                      onChange={setTab}
                      items={[
                        { value: "public", label: "Public log" },
                        { value: "private", label: "Private log" },
                      ]}
                    />
                  </div>
                  <TabPanel tabsId="log" value={tab} className="flex flex-col gap-2.5">
                    <LogEntry author="ada.lovelace" date="Fri 16:10">
                      It happened again at 15:52, right after the Teams call started.
                    </LogEntry>
                    <LogEntry author="agent.demo" date="Today 09:02" isPrivate={tab === "private"}>
                      Checked the gateway: DPD timeout is 60 s. Raising it to 300 s on the test profile first.
                    </LogEntry>
                  </TabPanel>
                  <Composer
                    value={draft}
                    onChange={setDraft}
                    onSend={() => setDraft("")}
                    label={tab === "public" ? "Public entry" : "Private entry"}
                    placeholder={tab === "public" ? "Write a public entry, the caller sees it" : "Write a private entry, only agents see it"}
                    sendLabel={tab === "public" ? "Add to public log" : "Add to private log"}
                    sendDisabled={!draft.trim()}
                    onAttach={() => {}}
                    attachTooltip="Add images. You can also paste or drop them in the entry. PNG, JPEG or GIF, up to 4 MB each."
                  />
                </div>
              </div>
            ) : (
              <EmptyState title="Select a ticket to see its details." className="flex-1" />
            )
          }
        />
      </AppShell>
    </div>
  );
}

/* ── the gallery ────────────────────────────────────────── */

function Gallery() {
  const [mode, setMode] = useState<ThemeMode>(() => {
    const t = new URLSearchParams(location.search).get("theme");
    return t === "light" || t === "dark" ? t : "system";
  });
  const dark = useSystemTheme(mode);
  const { toasts, show } = useToasts();

  const copy = (ref: string) => {
    void navigator.clipboard?.writeText(ref).catch(() => {});
    show(`Copied ${ref}`);
  };

  const [org, setOrg] = useState<string | null>("1");
  const [caller, setCaller] = useState<string | null>(null);
  const [code, setCode] = useState<string | null>("assistance");
  const [urgency, setUrgency] = useState<string | null>("4");
  const [kind, setKind] = useState<"UserRequest" | "Incident">("UserRequest");
  const [assign, setAssign] = useState(true);
  const [title, setTitle] = useState("");
  const [sheet, setSheet] = useState<null | "resolve" | "close" | "pending">(
    OPEN === "resolve" || OPEN === "close" ? OPEN : null,
  );
  useEffect(() => {
    const t = setTimeout(() => {
      const pick = (sel: string) => document.querySelector<HTMLElement>(sel);
      if (OPEN === "dropdown") {
        const el = pick('[role=combobox][type=button]');
        el?.focus();
        el?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
      } else if (OPEN === "search") {
        const el = pick("input[role=combobox]");
        el?.focus();
        el?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
      } else if (OPEN === "menu") {
        pick('[aria-haspopup=menu][aria-label^="View"], button[aria-haspopup=menu]:not([aria-label])')?.click();
      }
    }, 300);
    return () => clearTimeout(t);
  }, []);
  const [tried, setTried] = useState(false);
  const [solution, setSolution] = useState("");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [draft, setDraft] = useState("Restarted the client and re-imported the profile.");
  const [images, setImages] = useState(IMAGES);
  const [pplTab, setPplTab] = useState<"assigned" | "reported">("assigned");
  const [mbQuery, setMbQuery] = useState("");
  const [search, setSearch] = useState("VPN");
  const [cmdbSel, setCmdbSel] = useState<string | null>("web-01");

  const orgName = ORGS.find((o) => o.value === org)?.label;

  return (
    <div className="min-h-full bg-app text-ink">
      <header className="sticky top-0 z-40 flex h-12 items-center gap-3 border-b border-line bg-[color-mix(in_srgb,var(--app-bg)_85%,transparent)] px-6 backdrop-blur">
        <span className="text-[14px] font-semibold">iTop Wrap kit</span>
        <span className="text-[12px] text-ink-2">Beautiful UI, adapted · {dark ? "dark" : "light"}</span>
        <span className="flex-1" />
        <SegmentedControl
          label="Theme"
          size="sm"
          value={mode}
          onChange={setMode}
          options={[
            { value: "system", label: "System" },
            { value: "light", label: "Light" },
            { value: "dark", label: "Dark" },
          ]}
        />
      </header>

      <main className="mx-auto max-w-[1180px] px-6 pb-24">
        <Section id="shell" title="App shell, master/detail, ticket rows" source="Sidebar Nav · Task Rows · Search · Filter Table · Prompt Bar">
          <ShellDemo onCopy={copy} />
        </Section>

        <Section id="badges" title="Status, Needs you, priority, refs" source="Filter Table pill · ValuePill · Code Block copy">
          <Demo label="Status pills (text is the meaning)">
            {["New", "Assigned", "Pending", "Escalated TTR", "Escalated TTO", "Resolved", "Closed", "Waiting for approval"].map((s) => (
              <StatusPill key={s}>{s}</StatusPill>
            ))}
          </Demo>
          <Demo label="Needs you (the only teal)">
            <NeedsYouPill />
            <NeedsYouDot />
            <NeedsYouDot size={7} />
            <CountBadge count={3} tone="need" label="3 need you" />
            <CountBadge count={12} />
          </Demo>
          <Demo label="Priority marks: P1, P2, P3 (nothing)">
            <span className="flex items-center gap-1">P1 <PriorityMark priority={1} /></span>
            <span className="flex items-center gap-1">P2 <PriorityMark priority={2} /></span>
            <span className="flex items-center gap-1 text-ink-2">P3 <PriorityMark priority={3} /></span>
          </Demo>
          <Demo label="Ref chip (click to copy) · durations · copied chip">
            <RefChip value="R-000412" onCopy={copy} />
            <RefChip value="I-000088" size="sm" onCopy={copy} />
            <Duration>1h 20m</Duration>
            <Duration running>00:12:34</Duration>
            <CopiedChip>Copied R-000412</CopiedChip>
          </Demo>
        </Section>

        <Section id="people" title="People" source="EntityChip · Monogram (neutral disc)">
          <Demo label="Initials · unassigned · assignee · Assign to me">
            <InitialsAvatar name="Demo Agent" />
            <InitialsAvatar name="Grace Hopper" size={24} />
            <InitialsAvatar name="Ada Lovelace" size={44} />
            <UnassignedMark />
            <Assignee name="You" />
            <Assignee name="Agent Two" />
            <Assignee unassigned />
            <Assignee name="Agent Two" compact />
            <AssignToMePill ticketRef="R-000436" />
            <AssignToMePill ticketRef="R-000436" disabled />
          </Demo>
        </Section>

        <Section id="buttons" title="Buttons, icon buttons, tooltips, menus" source="Button atom · primitive-icon-button · GlideMenu · Records Table pickers">
          <Demo label="Variants">
            <Button variant="primary">Resolve ticket</Button>
            <Button variant="need">Assign to me</Button>
            <Button variant="secondary">Cancel</Button>
            <Button variant="ghost">Clear form</Button>
            <Button variant="quiet">See all of its tickets</Button>
            <Button variant="primary" busy>
              Resolve ticket
            </Button>
            <Button variant="primary" disabled tooltip="Changes to iTop are switched off in this build">
              Resolve ticket
            </Button>
            <Button size="xs" icon={<RefreshIcon />}>Retry</Button>
          </Demo>
          <Demo label="Block (sheet / sign in)" className="max-w-[380px]">
            <Button variant="need" size="block">Assign to me</Button>
            <Button variant="primary" size="block">Sign in to iTop</Button>
          </Demo>
          <Demo label="Icon buttons (tooltip + accessible name)">
            <IconButton label="Refresh" tooltip="Refresh (⌘R)" icon={<RefreshIcon />} />
            <IconButton label="Search" icon={<SearchIcon />} size="sm" />
            <IconButton label="Open in iTop on the web" icon={<ExternalIcon />} variant="field" size="sm" />
            <IconButton label="Refresh" icon={<RefreshIcon />} busy />
            <IconButton label="Refresh" icon={<RefreshIcon />} disabled />
            <MoreMenu
              items={[
                { id: "a", label: "Assign to someone else…" },
                { id: "p", label: "Set to pending…", disabled: true, hint: "Changes to iTop are switched off in this build" },
                { kind: "separator", id: "s" },
                { id: "c", label: "Copy ref R-000412", onSelect: () => copy("R-000412") },
                { id: "w", label: "Open in iTop on the web", icon: <ExternalIcon /> },
              ]}
            />
          </Demo>
          <Demo label="Filter chips (neutral; dark border when on)">
            <Menu
              label="Range"
              items={[
                { kind: "radio", id: "o", label: "Open tickets", checked: true },
                { kind: "radio", id: "7", label: "Opened in the last 7 days", checked: false },
              ]}
              trigger={(p) => <FilterChip {...p} icon={<CalendarIcon />} label="Open tickets" />}
            />
            <Menu
              label="Filter by organization"
              items={[{ kind: "radio", id: "a", label: "Finance and Accounting", checked: true }]}
              trigger={(p) => <FilterChip {...p} icon={<BuildingIcon />} label="Finance and Accounting, Headquarters and Subsidiaries" active />}
            />
          </Demo>
        </Section>

        <Section id="fields" title="Search, dropdowns, form fields" source="Search · Fine-tune Card select/segmented · Chat composer field">
          <Demo label="Search field (Esc clears)" className="max-w-[360px]">
            <SearchField value={search} onChange={setSearch} placeholder="R-000412 or VPN" className="w-full" />
            <SearchField value="" onChange={() => {}} placeholder="Search config items" busy className="w-full" />
          </Demo>
          <div className="grid max-w-[760px] grid-cols-2 gap-x-5">
            <Field label="Organization" required>
              {(ids) => (
                <DropdownField
                  id={ids.id}
                  aria-labelledby={`${ids.id}-label`}
                  items={ORGS}
                  value={org}
                  onChange={(v) => {
                    setOrg(v);
                    setCaller(null);
                  }}
                  searchable
                  searchPrompt="Search organizations"
                  noMatchText="No organization matches"
                />
              )}
            </Field>
            <Field label="Caller" required>
              {(ids) => (
                <DropdownField
                  id={ids.id}
                  aria-labelledby={`${ids.id}-label`}
                  items={org ? CALLERS : []}
                  value={caller}
                  onChange={setCaller}
                  searchable
                  placeholder={org ? "Choose…" : "Choose an organization first"}
                  searchPrompt={`Search people in ${orgName}`}
                  noMatchText={`No caller in ${orgName} matches`}
                />
              )}
            </Field>
            <Field label="Resolution code">
              {(ids) => <Select id={ids.id} aria-labelledby={`${ids.id}-label`} items={RESOLUTION} value={code} onChange={setCode} />}
            </Field>
            <Field label="Urgency">
              {(ids) => <Select id={ids.id} aria-labelledby={`${ids.id}-label`} items={URGENCY} value={urgency} onChange={setUrgency} />}
            </Field>
            <Field label="Team" hint="Only active teams in iTop are listed">
              {(ids) => <Select id={ids.id} aria-labelledby={`${ids.id}-label`} items={[]} value={null} onChange={() => {}} placeholder="Loading teams…" loading disabled />}
            </Field>
            <Field label="Service subcategory" required requiredError error="Fill in the subcategory first.">
              {(ids) => <Select id={ids.id} aria-labelledby={`${ids.id}-label`} items={[]} value={null} onChange={() => {}} placeholder="Choose a service first" disabled invalid />}
            </Field>
            <Field label="Title" required>
              {(ids) => <TextInput id={ids.id} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="What's wrong, in a few words" />}
            </Field>
            <Field label="Type" reserveErrorSpace={false}>
              <SegmentedControl
                label="Ticket type"
                value={kind}
                onChange={setKind}
                options={[
                  { value: "UserRequest", label: "User request" },
                  { value: "Incident", label: "Incident" },
                ]}
              />
            </Field>
            <Field label="Description" required className="col-span-2">
              {(ids) => <TextArea id={ids.id} placeholder="What the caller reports, and anything you've already checked." minHeight={100} />}
            </Field>
            <Checkbox checked={assign} onChange={setAssign} label="Assign to me" />
          </div>
        </Section>

        <Section id="sheets" title="Sheets and dialogs" source="Approval Card grammar on the window surface">
          <Demo label="Open one (Esc cancels, ⌘↩ confirms, Tab is trapped)">
            <Button onClick={() => (setTried(false), setSheet("resolve"))}>Resolve sheet</Button>
            <Button onClick={() => (setTried(false), setSheet("pending"))}>Set to pending sheet</Button>
            <Button onClick={() => setSheet("close")}>Close confirmation</Button>
          </Demo>
        </Section>

        <Section id="detail" title="Detail pieces" source="Context Cards chips · Approval Card fields">
          <Demo label="Key-value grid" className="block">
            <KeyValueGrid
              items={[
                { key: "s", label: "Status", value: "Production" },
                { key: "o", label: "Organization", value: "Demo Org A" },
                { key: "c", label: "Business criticality", value: "High" },
                { key: "l", label: "Location", value: "Paris DC 2" },
                { key: "os", label: "OS family", value: "Linux" },
                { key: "ip", label: "Management IP", value: <span className="mono-figures">10.0.12.4</span> },
                { key: "cpu", label: "CPU", value: "8 vCPU" },
                { key: "ram", label: "RAM", value: "32 GB" },
              ]}
            />
          </Demo>
          <Demo label="Details strip (with empty values)" className="block">
            <DetailStrip>
              <DetailItem icon={<PersonIcon />} label="Caller" />
              <DetailItem icon={<BuildingIcon />} label="Organization">Demo Org B</DetailItem>
              <DetailItem icon={<TeamIcon />} label="Team" emptyText="No team" />
              <Assignee unassigned />
            </DetailStrip>
          </Demo>
          <Demo label="Class icons · attachment chips · CI chips">
            <ClassIcon kind="UserRequest" />
            <ClassIcon kind="Incident" />
            <AttachmentChip name="invoice-2026-11.pdf" />
            <AttachmentChip name="busy.zip" busy />
            <AttachmentChip name="disabled.txt" disabled />
            <CIChip name="db-cluster-02" tooltip="Database server: open db-cluster-02" />
            <CIChip name="vpn-gw-01" variant="link" tooltip="Linked CI: open vpn-gw-01 in Config items" />
          </Demo>
          <Demo label="Tabs with counts" className="block">
            <Tabs
              id="ppl"
              label="Tickets"
              value={pplTab}
              onChange={setPplTab}
              items={[
                { value: "assigned", label: "Assigned to them", count: 12 },
                { value: "reported", label: "Reported by them", count: 3 },
              ]}
            />
          </Demo>
          <Demo label="Log entries" className="block max-w-[620px]">
            <div className="flex flex-col gap-2.5">
              <LogEntry author="grace.hopper" date="2 Nov 09:00">
                The relay log shows <code className="mono-figures text-[12px]">552 5.3.4 Message size exceeds fixed limit</code>.
              </LogEntry>
              <LogEntry author="agent.demo" date="Yesterday 17:45" isPrivate>
                Only agents see this. Change request CR-2231 is waiting for CAB.
              </LogEntry>
            </div>
          </Demo>
        </Section>

        <Section id="composer" title="Composer" source="Prompt Bar (Rounded) · Chat composer">
          <div className="grid max-w-[1000px] grid-cols-2 items-start gap-5">
            <Composer
              value={draft}
              onChange={setDraft}
              onSend={() => setBusy(true)}
              label="Public entry"
              placeholder="Write a public entry, the caller sees it"
              sendLabel="Add to public log"
              images={images}
              onRemoveImage={(id) => setImages((all) => all.filter((i) => i.id !== id))}
              onAttach={() => setImages(IMAGES)}
              attachTooltip="Add images. You can also paste or drop them in the entry. PNG, JPEG or GIF, up to 4 MB each."
              busy={busy}
              status={busy ? "Uploading 2 images to iTop" : undefined}
            />
            <Composer
              value="fail"
              onChange={() => {}}
              onSend={() => {}}
              label="Private entry"
              sendLabel="Add to private log"
              onAttach={() => {}}
              status="iTop didn't save the entry. The server answered with HTTP 500. Your draft is kept."
              statusTone="error"
            />
            <Composer
              value=""
              onChange={() => {}}
              onSend={() => {}}
              label="Public entry"
              placeholder="Write a public entry, the caller sees it"
              sendLabel="Add to public log"
              sendDisabled
              readOnly
              onAttach={() => {}}
              attachTooltip="Adding images is switched off"
              sendTooltip="Changes to iTop are switched off in this build"
              status="Adding entries is switched off in this build, so nothing is sent to iTop."
            />
            <div className="flex items-start gap-2">
              <Button size="xs" onClick={() => setBusy(false)}>
                Reset busy
              </Button>
            </div>
          </div>
        </Section>

        <Section id="timer" title="Timer button" source="Hand-built in the header-control style">
          <Demo label="Idle · running · disabled · saving · web stopwatch">
            <TimerButton state="idle" onToggle={() => {}} />
            <TimerButton state="running" startedAt={RUNNING_SINCE} onToggle={() => {}} />
            <TimerButton state="idle" disabled />
            <TimerButton state="idle" busy />
            <TimerButton state="web" webSince="09:14" />
          </Demo>
        </Section>

        <Section id="states" title="Loading, empty, error, notices" source="Loading State · Search empty state">
          <div className="grid grid-cols-3 gap-4">
            <div className="rounded-card bg-surface shadow-card"><LoadingState label="Loading tickets from iTop" /></div>
            <div className="rounded-card bg-surface shadow-card"><EmptyState title="Nothing waiting on you. New assignments show up here." /></div>
            <div className="rounded-card bg-surface shadow-card">
              <ErrorState message="Can't reach itop.example.com. Check your connection, then retry." onRetry={() => {}} />
            </div>
            <div className="rounded-card bg-surface shadow-card">
              <EmptyState icon={<SearchIcon />} title="Search by name, for example a server or application." />
            </div>
            <div className="rounded-card bg-surface shadow-card">
              <EmptyState title="None of your tickets are for Demo Org C." action={<Button variant="secondary" size="sm">See all of its tickets</Button>} />
            </div>
            <div className="flex items-center justify-center gap-2 rounded-card bg-surface text-[12.5px] text-ink-2 shadow-card">
              <Spinner /> Loading the ticket from iTop
            </div>
          </div>
          <div className="flex max-w-[560px] flex-col gap-2">
            <NoticeBanner onDismiss={() => {}}>R-000412 is assigned to you.</NoticeBanner>
            <NoticeBanner icon={<SearchIcon />}>Looking up R-000512 in iTop…</NoticeBanner>
            <NoticeBanner tone="error" onDismiss={() => {}}>Couldn't refresh: Can't reach itop.example.com. Check your connection, then retry.</NoticeBanner>
            <NoticeBanner tone="error" icon={<StopwatchIcon />} onRetry={() => {}} onDismiss={() => {}} dismissLabel="Discard this time">
              Couldn't save 0:42 to R-000398. The server answered with HTTP 503.
            </NoticeBanner>
            <NoticeBanner icon={<StopwatchIcon />} onDismiss={() => {}}>Resolving R-000398 stopped its timer. Saved 0:12 to R-000398.</NoticeBanner>
          </div>
        </Section>

        <Section id="lists" title="Lists, tables, pager" source="Filter Table · Records Table footer · Task Rows">
          <div className="grid grid-cols-[320px_1fr] items-start gap-5">
            <div className="overflow-hidden rounded-card bg-surface shadow-card">
              <Listbox label="Config items" selectedId={cmdbSel} onSelect={setCmdbSel}>
                <ListSection title="First 50 results" count="">
                  <SimpleRow id="web-01" title="web-01" subtitle="Server · Demo Org A" mono />
                  <SimpleRow id="db-cluster-02" title="db-cluster-02" subtitle="Database server · Demo Org A" mono />
                  <SimpleRow id="org" title="Demo Org A" subtitle="DOA" trailing="12" />
                  <SimpleRow id="person" title="Ada Lovelace" subtitle="Caller · Demo Org A" leading={<InitialsAvatar name="Ada Lovelace" size={28} />} />
                </ListSection>
              </Listbox>
              <ListFooter pager={<Pager first={1} last={4} total={4} onPrevious={() => {}} onNext={() => {}} noun={{ one: "result", other: "results" }} />} updatedLabel="Updated just now" />
            </div>
            <TicketTable
              onOpen={() => {}}
              groups={[
                {
                  title: "Escalated TTR",
                  count: 1,
                  rows: [{ id: "a", ticketRef: "R-000412", title: "VPN drops every 20 minutes", priority: 1, needsYou: true, caller: "Ada Lovelace", assigneeName: "You" }],
                },
                {
                  title: "New",
                  count: 2,
                  rows: [
                    { id: "b", ticketRef: "I-000102", title: "Badge reader at north entrance offline", priority: 2, caller: "Alan Turing", unassigned: true },
                    { id: "c", ticketRef: "R-000440", title: "Request a second monitor", priority: 4, caller: "Grace Hopper", assigneeName: "Agent Two" },
                  ],
                },
              ]}
            />
          </div>
          <Demo label="Pager states">
            <Pager first={21} last={40} total={556} onPrevious={() => {}} onNext={() => {}} />
            <Pager first={1} last={20} total={26} onPrevious={() => {}} onNext={() => {}} />
            <Pager first={1} last={1} total={1} onPrevious={() => {}} onNext={() => {}} />
            <ViewSummary>All tickets, any time · Last update, newest first · 50 per page</ViewSummary>
          </Demo>
        </Section>

        <Section id="menubar" title="Menu bar panel" source="Task Rows (List) · GlideMenu · Search">
          <div className="w-fit overflow-hidden rounded-[14px] shadow-overlay">
            <MenuBarPanel>
              <MenuBarTopRow query={mbQuery} onQueryChange={setMbQuery} onNewTicket={() => {}} />
              <div className="px-2 pt-2">
                <NoticeBanner icon={<StopwatchIcon />} onDismiss={() => {}}>Saved 0:42 to R-000405.</NoticeBanner>
              </div>
              <div className="kit-scroll max-h-[420px] overflow-y-auto pb-1.5">
                <MenuBarSection title="Running">
                  <MenuBarTicketRow ticketRef="R-000398" title="Printer on 3rd floor prints blank pages" runningSince={RUNNING_SINCE} timer="stop" />
                </MenuBarSection>
                <MenuBarSection title="Escalated TTR" count={2}>
                  <MenuBarTicketRow ticketRef="R-000412" title="VPN drops every 20 minutes for the whole finance floor" needsYou total="1h 20m" timer="none" />
                  <MenuBarTicketRow ticketRef="I-000088" title="Mail relay rejects messages larger than 10 MB" needsYou total="25m" timer="none" />
                </MenuBarSection>
                <MenuBarSection title="Assigned" count={2}>
                  <MenuBarTicketRow ticketRef="R-000405" title="Shared drive permissions for the audit team" total="2h" timer="start" timerTooltip="Stops the timer on R-000398, saves its time, and starts one here" />
                  <MenuBarTicketRow ticketRef="I-000091" title="Wi-Fi in meeting room Ganymede is slow" timer="start" />
                </MenuBarSection>
              </div>
              <MenuBarFooter onRefresh={() => {}} updatedLabel="Updated 2 minutes ago" onOpenApp={() => {}} />
            </MenuBarPanel>
          </div>
        </Section>
      </main>

      <Dialog
        open={sheet === "resolve"}
        title="Resolve R-000412"
        subtitle="VPN drops every 20 minutes for the whole finance floor"
        onCancel={() => setSheet(null)}
        onConfirm={() => {
          setTried(true);
          if (solution.trim()) setSheet(null);
        }}
        confirmLabel="Resolve ticket"
        error={tried && !solution.trim() ? "Fill in the solution first." : undefined}
      >
        <Field label="Service" required>
          {(ids) => <Select id={ids.id} aria-labelledby={`${ids.id}-label`} items={[{ value: "1", label: "Network" }, { value: "2", label: "Workplace" }]} value="1" onChange={() => {}} />}
        </Field>
        <Field label="Service subcategory" required>
          {(ids) => <Select id={ids.id} aria-labelledby={`${ids.id}-label`} items={[{ value: "a", label: "VPN access" }]} value="a" onChange={() => {}} />}
        </Field>
        <Field label="Resolution code">
          {(ids) => <Select id={ids.id} aria-labelledby={`${ids.id}-label`} items={RESOLUTION} value={code} onChange={setCode} />}
        </Field>
        <Field label="Solution" required requiredError={tried && !solution.trim()} error={tried && !solution.trim() ? "Describe what fixed it. The caller sees this." : undefined}>
          {(ids) => (
            <TextArea
              id={ids.id}
              data-autofocus
              aria-describedby={ids.describedBy}
              aria-required
              invalid={ids.invalid}
              value={solution}
              onChange={(e) => setSolution(e.target.value)}
              placeholder="What fixed it, in words the caller understands."
              minHeight={110}
            />
          )}
        </Field>
        <p className="flex items-center gap-1.5 text-[12px] text-ink-2">
          <StopwatchIcon className="size-3.5" /> Your timer stops and saves 0:12 to this ticket.
        </p>
      </Dialog>

      <Dialog
        open={sheet === "pending"}
        title="Set R-000412 to pending"
        subtitle="VPN drops every 20 minutes for the whole finance floor"
        onCancel={() => setSheet(null)}
        onConfirm={() => {
          setTried(true);
          if (reason.trim()) setSheet(null);
        }}
        confirmLabel="Set to pending"
      >
        <Field label="Reason" required requiredError={tried && !reason.trim()} error={tried && !reason.trim() ? "Enter why the ticket is waiting." : undefined}>
          {(ids) => (
            <TextArea
              id={ids.id}
              aria-describedby={ids.describedBy}
              invalid={ids.invalid}
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="What are you waiting for? For example the caller's reply or a part."
            />
          )}
        </Field>
      </Dialog>

      <Dialog
        open={sheet === "close"}
        role="alertdialog"
        title="Close R-000412?"
        subtitle="A closed ticket can't be reopened from the app."
        onCancel={() => setSheet(null)}
        onConfirm={() => setSheet(null)}
        confirmLabel="Close ticket"
        width={360}
      />

      <ToastRegion toasts={toasts} />
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Gallery />
  </StrictMode>,
);
