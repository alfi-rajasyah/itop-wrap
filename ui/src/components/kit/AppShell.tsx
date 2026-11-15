import type { KeyboardEvent, ReactNode } from "react";
import GlideMenu from "@/components/primitives/GlideMenu";
import { cn } from "@/lib/utils";
import { InitialsAvatar } from "./Avatar";
import { CountBadge } from "./Badges";
import { ChevronUpDownIcon } from "./icons";
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
 * 78px kept clear for the traffic lights; count badges (teal only
 * when it counts "needs you"); keyboard hints (⌘1…); ↑/↓ between
 * items; the signed-in user's initials and menu in the footer.
 * ───────────────────────────────────────────────────────── */

export type SidebarItem = {
  id: string;
  label: string;
  icon: ReactNode;
  count?: number;
  /** "need" = the teal Needs-you capsule (My tickets). */
  countTone?: "neutral" | "need";
  /** Accessible/tooltip text for the count ("3 need you"). */
  countLabel?: string;
  /** "⌘1". Shown on hover and in the tooltip. */
  shortcut?: string;
};

export type SidebarSection = { id: string; title?: string; items: SidebarItem[] };

export type SidebarUser = { name: string; detail?: string; initials?: string; menu: MenuEntry[] };

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
  width = 216,
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
  label?: string;
  className?: string;
}) {
  const onKey = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const items = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("[data-nav-item]"));
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    e.preventDefault();
    items[(i + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length]?.focus();
  };

  return (
    <aside
      aria-label={label}
      className={cn("flex h-full min-h-0 shrink-0 flex-col border-r border-line bg-sidebar", className)}
      style={{ width }}
    >
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
        <div className="mx-2 border-t border-line py-2">
          <Menu
            label="Account"
            items={user.menu}
            placement="top-start"
            minWidth={200}
            trigger={({ ref, ...props }) => (
              <button
                ref={ref}
                type="button"
                aria-label={`Account: ${user.name}`}
                className="flex h-10 w-full items-center gap-2 rounded-[8px] px-2 text-left transition-[background-color,transform] duration-100 hover:bg-hover-2 active:scale-[0.99] aria-expanded:bg-hover-2"
                {...props}
              >
                <InitialsAvatar name={user.name} initials={user.initials} size={24} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[13px] font-medium text-ink">{user.name}</span>
                  {user.detail ? <span className="block truncate text-[11.5px] text-ink-2">{user.detail}</span> : null}
                </span>
                <ChevronUpDownIcon className="size-4 shrink-0 text-ink-2" />
              </button>
            )}
          />
        </div>
      ) : null}
    </aside>
  );
}

/** The window: sidebar on the left, the rest on the right. Fills its parent (h-screen). */
export function AppShell({ sidebar, children, className }: { sidebar: ReactNode; children: ReactNode; className?: string }) {
  return (
    <div className={cn("flex h-full min-h-0 w-full overflow-hidden bg-app text-ink", className)}>
      {sidebar}
      <main className="flex min-h-0 min-w-0 flex-1">{children}</main>
    </div>
  );
}
