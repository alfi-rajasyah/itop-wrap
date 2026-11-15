import {
  forwardRef,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
  type Ref,
} from "react";
import GlideMenu from "@/components/primitives/GlideMenu";
import { cn } from "@/lib/utils";
import { CheckIcon, ChevronDownIcon, MoreIcon } from "./icons";
import { IconButton } from "./IconButton";
import { Popover, type Placement } from "./Popover";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * MENU — a real role="menu" built on Beautiful UI's menu card and
 * GlideMenu highlight (Records Table's ConfigPicker / InputPicker:
 * 32px rows, 8px radius, check on the right, 11.5px section labels).
 * Used for the "…" More menu, the View menu, the org/team filters
 * and the account menu.
 *
 * Keyboard: Enter/Space/↓ on the trigger opens on the first item,
 * ↑ on the last; ↑/↓/Home/End move, a letter jumps, Enter/Space
 * picks, Esc closes and returns focus, Tab closes.
 * ───────────────────────────────────────────────────────── */

export type MenuEntry =
  | {
      kind?: "item";
      id: string;
      label: ReactNode;
      /** Plain text for typeahead when `label` isn't a string. */
      textValue?: string;
      icon?: ReactNode;
      shortcut?: string;
      disabled?: boolean;
      /** Tooltip, e.g. why it's disabled. */
      hint?: string;
      onSelect?: () => void;
    }
  | {
      kind: "checkbox";
      id: string;
      label: ReactNode;
      textValue?: string;
      checked: boolean;
      disabled?: boolean;
      onCheckedChange?: (checked: boolean) => void;
      /** Keep the menu open after toggling. Default true. */
      keepOpen?: boolean;
    }
  | {
      kind: "radio";
      id: string;
      label: ReactNode;
      textValue?: string;
      checked: boolean;
      disabled?: boolean;
      onSelect?: () => void;
    }
  | { kind: "separator"; id: string }
  | { kind: "section"; id: string; label: ReactNode };

export type MenuTriggerProps = {
  ref: Ref<HTMLButtonElement>;
  onClick: () => void;
  onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => void;
  "aria-haspopup": "menu";
  "aria-expanded": boolean;
  "aria-controls"?: string;
};

type Actionable = Exclude<MenuEntry, { kind: "separator" } | { kind: "section" }>;

export function Menu({
  label,
  items,
  trigger,
  placement = "bottom-start",
  minWidth = 200,
  footer,
  onOpenChange,
}: {
  /** Accessible name of the menu ("More actions", "View options"). */
  label: string;
  items: MenuEntry[];
  trigger: (props: MenuTriggerProps) => ReactNode;
  placement?: Placement;
  minWidth?: number;
  /** Quiet text under the items (not focusable). */
  footer?: ReactNode;
  onOpenChange?: (open: boolean) => void;
}) {
  const menuId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [open, setOpenState] = useState(false);

  const setOpen = (next: boolean) => {
    setOpenState(next);
    onOpenChange?.(next);
  };

  const rows = () => Array.from(menuRef.current?.querySelectorAll<HTMLButtonElement>("[data-menu-row]:not(:disabled)") ?? []);
  const focusAt = (index: "first" | "last" | number) => {
    const list = rows();
    if (!list.length) return;
    const i = index === "first" ? 0 : index === "last" ? list.length - 1 : (index + list.length) % list.length;
    list[i]?.focus();
  };

  const openWith = (where: "first" | "last") => {
    setOpen(true);
    // wait for the portal to mount and be positioned
    requestAnimationFrame(() => requestAnimationFrame(() => focusAt(where)));
  };

  const close = (refocus = true) => {
    setOpen(false);
    if (refocus) triggerRef.current?.focus();
  };

  const onTriggerKey = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      openWith("first");
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      openWith("last");
    }
  };

  const onMenuKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const list = rows();
    const current = list.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowDown") {
      e.preventDefault();
      focusAt(current < 0 ? "first" : current + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      focusAt(current < 0 ? "last" : current - 1);
    } else if (e.key === "Home") {
      e.preventDefault();
      focusAt("first");
    } else if (e.key === "End") {
      e.preventDefault();
      focusAt("last");
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === "Tab") {
      e.preventDefault();
      close();
    } else if (e.key.length === 1 && /\S/.test(e.key) && !e.metaKey && !e.ctrlKey) {
      const ch = e.key.toLowerCase();
      const start = current + 1;
      for (let k = 0; k < list.length; k++) {
        const el = list[(start + k) % list.length]!;
        if ((el.dataset.text ?? el.textContent ?? "").trim().toLowerCase().startsWith(ch)) {
          el.focus();
          break;
        }
      }
    }
  };

  const activate = (entry: Actionable) => {
    if (entry.disabled) return;
    if (entry.kind === "checkbox") {
      entry.onCheckedChange?.(!entry.checked);
      if (entry.keepOpen === false) close();
      return;
    }
    close();
    entry.onSelect?.();
  };

  return (
    <>
      {trigger({
        ref: triggerRef,
        onClick: () => (open ? close(false) : setOpen(true)),
        onKeyDown: onTriggerKey,
        "aria-haspopup": "menu",
        "aria-expanded": open,
        "aria-controls": open ? menuId : undefined,
      })}
      <Popover
        anchorRef={triggerRef}
        open={open}
        onDismiss={() => close(false)}
        placement={placement}
        floatingRef={menuRef}
        maxHeight={440}
        style={{ minWidth }}
      >
        <div id={menuId} role="menu" aria-label={label} onKeyDown={onMenuKey} className="outline-none">
          <GlideMenu className="flex flex-col gap-px" highlightClassName="inset-x-0 rounded-[8px] bg-hover-2">
            {items.map((entry) => {
              if (entry.kind === "separator") return <div key={entry.id} role="separator" className="mx-1 my-1 h-px bg-line" />;
              if (entry.kind === "section")
                return (
                  <div key={entry.id} role="presentation" className="px-2 pb-0.5 pt-1.5 text-[11.5px] font-medium text-ink-2 first:pt-0.5">
                    {entry.label}
                  </div>
                );
              const role = entry.kind === "checkbox" ? "menuitemcheckbox" : entry.kind === "radio" ? "menuitemradio" : "menuitem";
              const checked = entry.kind === "checkbox" || entry.kind === "radio" ? entry.checked : undefined;
              const row = (
                <button
                  key={entry.id}
                  data-menu-row
                  data-text={entry.textValue ?? (typeof entry.label === "string" ? entry.label : undefined)}
                  type="button"
                  role={role}
                  aria-checked={checked}
                  disabled={entry.disabled}
                  tabIndex={-1}
                  onClick={() => activate(entry)}
                  onMouseMove={(e) => {
                    if (document.activeElement !== e.currentTarget) e.currentTarget.focus({ preventScroll: true });
                  }}
                  className="relative z-10 flex h-8 w-full items-center gap-2 rounded-[8px] px-2 text-left text-[13px] text-ink outline-none disabled:opacity-40"
                >
                  {entry.kind === "checkbox" ? (
                    <span
                      className={cn(
                        "flex size-4 shrink-0 items-center justify-center rounded-[5px] border transition-colors duration-100",
                        entry.checked ? "border-ink bg-ink text-surface" : "border-line-strong bg-surface text-transparent",
                      )}
                    >
                      <CheckIcon className="size-3" strokeWidth={2.6} />
                    </span>
                  ) : (!entry.kind || entry.kind === "item") && entry.icon ? (
                    <span className="flex size-4 shrink-0 items-center justify-center text-ink-2 [&_svg]:size-4">{entry.icon}</span>
                  ) : null}
                  <span className="min-w-0 flex-1 truncate">{entry.label}</span>
                  {(!entry.kind || entry.kind === "item") && entry.shortcut ? (
                    <kbd className="shrink-0 font-sans text-[11.5px] text-ink-2">{entry.shortcut}</kbd>
                  ) : null}
                  {entry.kind === "radio" ? (
                    <span className={cn("shrink-0 text-ink", entry.checked ? "" : "invisible")}>
                      <CheckIcon className="size-3.5" strokeWidth={2.2} />
                    </span>
                  ) : null}
                </button>
              );
              return (!entry.kind || entry.kind === "item") && entry.hint ? (
                <Tooltip key={entry.id} content={entry.hint} side="top" className="flex w-full">
                  {row}
                </Tooltip>
              ) : (
                row
              );
            })}
          </GlideMenu>
          {footer ? <div className="mt-1 border-t border-line px-2 pb-0.5 pt-1.5 text-[11.5px] text-ink-2">{footer}</div> : null}
        </div>
      </Popover>
    </>
  );
}

/** The "…" More menu trigger (icon-only, "More actions"). */
export function MoreMenu({
  items,
  label = "More actions",
  variant = "field",
  size = "sm",
}: {
  items: MenuEntry[];
  label?: string;
  variant?: "quiet" | "field";
  size?: "xs" | "sm" | "md";
}) {
  return (
    <Menu
      label={label}
      items={items}
      placement="bottom-end"
      trigger={({ ref, ...props }) => <IconButton ref={ref} label={label} icon={<MoreIcon />} variant={variant} size={size} {...props} />}
    />
  );
}

/**
 * Filter chip trigger: icon + label, neutral grey border when off and a dark border when
 * on (never the accent colour, DESIGN.md). Adapted from Filter Table's chips. Pass the
 * Menu trigger props through.
 */
export const FilterChip = forwardRef<
  HTMLButtonElement,
  {
    icon?: ReactNode;
    label: ReactNode;
    active?: boolean;
    tooltip?: ReactNode;
    "aria-label"?: string;
    className?: string;
    maxWidth?: number;
  } & Omit<MenuTriggerProps, "ref">
>(function FilterChip({ icon, label, active = false, tooltip, className, maxWidth = 200, ...props }, ref) {
  const button = (
    <button
      ref={ref}
      type="button"
      className={cn(
        "inline-flex h-7 min-w-0 shrink-0 items-center gap-1.5 rounded-full bg-surface pl-2 pr-1.5 text-[12.5px] font-medium text-ink",
        "transition-[background-color,box-shadow,transform] duration-150 hover:bg-inset active:scale-[0.97] aria-expanded:bg-hover",
        active ? "shadow-[0_0_0_1.5px_var(--ink)]" : "shadow-btn",
        className,
      )}
      style={{ maxWidth }}
      {...props}
    >
      {icon ? <span className="flex shrink-0 text-ink-2 [&_svg]:size-[15px]">{icon}</span> : null}
      <span className="min-w-0 truncate">{label}</span>
      <ChevronDownIcon className="size-3.5 shrink-0 text-ink-2" strokeWidth={2} />
    </button>
  );
  return tooltip ? (
    <Tooltip content={tooltip} describe={false}>
      {button}
    </Tooltip>
  ) : (
    button
  );
});
