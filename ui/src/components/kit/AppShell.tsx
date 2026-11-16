import { useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent, type ReactNode } from "react";
import GlideMenu from "@/components/primitives/GlideMenu";
import { cn } from "@/lib/utils";
import { initialsOf } from "./Avatar";
import { CountBadge } from "./Badges";
import { Menu, type MenuEntry } from "./Menu";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * APP SHELL — sidebar + content, adapted from Sidebar Nav.
 *
 * Kept from Sidebar Nav: the 32px rows with a 20px icon slot, the
 * GlideMenu hover layer that slides between rows, the hover-2
 * selected tint, the quiet section label and the bordered footer.
 * Dropped: workspace switcher, chat search, recents and the collapse
 * animation (not in the app); the paid Central Icons (callers pass
 * iconoir icons).
 * Added: a 52px drag region for the macOS overlay title bar, with
 * 78px kept clear for the traffic lights; count badges (red only
 * when it counts "needs you"); keyboard hints (⌘1…); ↑/↓ between
 * items; the signed-in user's initials and menu in the footer.
 * ───────────────────────────────────────────────────────── */

export type SidebarItem = {
  id: string;
  label: string;
  icon: ReactNode;
  count?: number;
  /** "need" = the red Needs-you capsule (My tickets). */
  countTone?: "neutral" | "need";
  /** Accessible/tooltip text for the count ("3 need you"). */
  countLabel?: string;
  /** "⌘1". Shown on hover and in the tooltip. */
  shortcut?: string;
};

export type SidebarSection = { id: string; title?: string; items: SidebarItem[] };

export type SidebarUser = {
  name: string;
  detail?: string;
  initials?: string;
  menu: MenuEntry[];
  /** Accessible name of the account button; defaults to "Account: <name>". */
  ariaLabel?: string;
  /** Tooltip of the account button. */
  tooltip?: string;
};

export const TRAFFIC_LIGHTS_WIDTH = 78;

function SidebarRow({ item, selected, onSelect }: { item: SidebarItem; selected: boolean; onSelect: () => void }) {
  const count =
    item.count != null && (item.countTone !== "need" || item.count > 0) ? (
      <CountBadge count={item.count} tone={item.countTone} label={item.countLabel} />
    ) : null;
  return (
    <button
      data-row
      data-nav-item
      type="button"
      aria-current={selected ? "page" : undefined}
      aria-keyshortcuts={item.shortcut?.replace("⌘", "Meta+")}
      onClick={onSelect}
      className={cn(
        "group/item relative z-10 mx-2 flex h-8 items-center rounded-[8px] px-2 text-left",
        "transition-[background-color,color,transform] duration-150 active:scale-[0.98]",
        selected ? "bg-hover-2 group-hover/glide:bg-transparent" : "",
      )}
    >
      <span className={cn("flex size-5 shrink-0 items-center justify-center [&_svg]:size-[17px]", selected ? "text-ink" : "text-ink-2")}>
        {item.icon}
      </span>
      <span className={cn("ml-2 min-w-0 flex-1 truncate text-[13px] font-medium", selected ? "text-ink" : "text-ink-2")}>{item.label}</span>
      {item.shortcut ? (
        <kbd
          aria-hidden
          className={cn(
            "ml-1 shrink-0 font-sans text-[11px] text-ink-2 opacity-0 transition-opacity duration-150 group-hover/item:opacity-100 group-focus-visible/item:opacity-100",
            count && "group-hover/item:hidden group-focus-visible/item:hidden",
          )}
        >
          {item.shortcut}
        </kbd>
      ) : null}
      {count ? (
        item.countLabel ? (
          <Tooltip content={item.countLabel} describe={false}>
            {count}
          </Tooltip>
        ) : (
          count
        )
      ) : null}
    </button>
  );
}

export function Sidebar({
  sections,
  selectedId,
  onSelect,
  user,
  header,
  width = 200,
  minWidth = 180,
  maxWidth = 260,
  onWidthChange,
  label = "Sections",
  className,
}: {
  sections: SidebarSection[];
  selectedId: string;
  onSelect: (id: string) => void;
  user?: SidebarUser;
  /** Right of the traffic lights, in the drag region (e.g. a refresh state). */
  header?: ReactNode;
  width?: number;
  minWidth?: number;
  maxWidth?: number;
  /** With it, the right edge is a divider; called with the new width when a drag ends. */
  onWidthChange?: (width: number) => void;
  label?: string;
  className?: string;
}) {
  const [dragWidth, setDragWidth] = useState<number | null>(null);
  const drag = useRef<{ x: number; w: number } | null>(null);
  const clamp = (w: number) => Math.round(Math.min(maxWidth, Math.max(minWidth, w)));
  const shown = dragWidth ?? width;

  // KEY-9, SHELL-3: like the native sidebar list, ↑/↓ move the selection itself (no wrap).
  const onKey = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const items = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("[data-nav-item]"));
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    e.preventDefault();
    const next = items[Math.min(items.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)))];
    if (!next || next === items[i]) return;
    next.focus();
    next.click();
  };

  const onDividerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, w: shown };
  };
  const onDividerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (drag.current) setDragWidth(clamp(drag.current.w + e.clientX - drag.current.x));
  };
  const onDividerUp = () => {
    if (!drag.current) return;
    drag.current = null;
    if (dragWidth !== null && dragWidth !== width) onWidthChange?.(dragWidth);
    setDragWidth(null);
  };
  const onDividerKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 40 : 16;
    const to =
      e.key === "ArrowLeft" ? width - step : e.key === "ArrowRight" ? width + step : e.key === "Home" ? minWidth : e.key === "End" ? maxWidth : null;
    if (to === null) return;
    e.preventDefault();
    if (clamp(to) !== width) onWidthChange?.(clamp(to));
  };

  return (
    <aside
      aria-label={label}
      className={cn("relative flex h-full min-h-0 shrink-0 flex-col border-r border-line bg-sidebar", className)}
      style={{ width: shown }}
    >
      {onWidthChange ? (
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize the sidebar"
          aria-valuenow={shown}
          aria-valuemin={minWidth}
          aria-valuemax={maxWidth}
          tabIndex={0}
          onPointerDown={onDividerDown}
          onPointerMove={onDividerMove}
          onPointerUp={onDividerUp}
          onPointerCancel={onDividerUp}
          onKeyDown={onDividerKey}
          className="absolute inset-y-0 -right-[4px] z-20 w-[7px] cursor-col-resize touch-none outline-none focus-visible:bg-[var(--focus)]/40"
        />
      ) : null}
      <div data-tauri-drag-region className="flex h-[52px] shrink-0 items-center pr-2" style={{ paddingLeft: TRAFFIC_LIGHTS_WIDTH }}>
        <div data-tauri-drag-region className="flex min-w-0 flex-1 items-center justify-end gap-1">
          {header}
        </div>
      </div>

      <nav aria-label={label} onKeyDown={onKey} className="kit-scroll min-h-0 flex-1 overflow-y-auto pb-3">
        {sections.map((section) => (
          <div key={section.id} className="mb-3">
            {section.title ? (
              <div className="mx-2 flex h-7 items-center px-2 text-[11.5px] font-medium text-ink-2">{section.title}</div>
            ) : null}
            <GlideMenu
              rowSelector="[data-row]"
              highlightClassName="right-2 left-2 rounded-[8px] bg-hover-2"
              className="group/glide flex flex-col gap-px"
            >
              {section.items.map((item) => (
                <SidebarRow key={item.id} item={item} selected={item.id === selectedId} onSelect={() => onSelect(item.id)} />
              ))}
            </GlideMenu>
          </div>
        ))}
      </nav>

      {user ? (
        // AUTH-27: just the 28 pt initials circle (11 pt semibold), left-aligned above a divider.
        <div className="flex border-t border-line px-3 py-2.5">
          <Menu
            label="Account"
            items={user.menu}
            placement="top-start"
            minWidth={200}
            trigger={({ ref, ...props }) => {
              const button = (
                <button
                  ref={ref}
                  type="button"
                  aria-label={user.ariaLabel ?? `Account: ${user.name}`}
                  className="flex size-7 items-center justify-center rounded-full bg-hover-2 text-[11px] font-semibold leading-none text-ink-2 transition-[background-color,transform] duration-100 hover:bg-hover active:scale-95 aria-expanded:bg-hover"
                  {...props}
                >
                  {user.initials ?? initialsOf(user.name)}
                </button>
              );
              return user.tooltip ? (
                <Tooltip content={user.tooltip} side="top" describe={false}>
                  {button}
                </Tooltip>
              ) : (
                button
              );
            }}
          />
        </div>
      ) : null}
    </aside>
  );
}

/** The window: sidebar on the left, the rest on the right. Fills its parent (h-screen). */
export function AppShell({
  sidebar,
  children,
  className,
  sidebarVisible = true,
  sidebarToggle,
}: {
  sidebar: ReactNode;
  children: ReactNode;
  className?: string;
  /** False: the sidebar is collapsed (View ▸ Hide Sidebar). */
  sidebarVisible?: boolean;
  /** The sidebar toggle button; with the sidebar collapsed it sits right of the traffic lights. */
  sidebarToggle?: ReactNode;
}) {
  // With the sidebar collapsed the first pane's title row starts after the traffic lights and
  // the toggle (PaneHeader reads --titlebar-inset).
  const inset = { "--titlebar-inset": `${TRAFFIC_LIGHTS_WIDTH + 36}px` } as CSSProperties;
  return (
    <div className={cn("relative flex h-full min-h-0 w-full overflow-hidden bg-app text-ink", className)} style={sidebarVisible ? undefined : inset}>
      {/* Kept mounted so collapsing can slide: the grid column eases between the sidebar's
          width (1fr of a max-content wrapper) and 0, clipping the sidebar as it goes. */}
      <div
        className="grid shrink-0 transition-[grid-template-columns] duration-200 ease-[cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none"
        style={{ gridTemplateColumns: sidebarVisible ? "1fr" : "0fr" }}
        inert={!sidebarVisible}
        aria-hidden={!sidebarVisible || undefined}
        data-sidebar-visible={sidebarVisible}
      >
        <div className="flex min-h-0 min-w-0 overflow-hidden">{sidebar}</div>
      </div>
      {!sidebarVisible && sidebarToggle ? (
        <div className="absolute top-[12px] z-30" style={{ left: TRAFFIC_LIGHTS_WIDTH }}>
          {sidebarToggle}
        </div>
      ) : null}
      <main className="flex min-h-0 min-w-0 flex-1">{children}</main>
    </div>
  );
}
