import type { KeyboardEvent, ReactNode } from "react";
import { cn } from "@/lib/utils";

/* ─────────────────────────────────────────────────────────
 * TABS — role="tablist" with automatic activation. The look is
 * Chat's header tabs (13px, 6px radius, the active one on a field
 * tint), with the inactive ones in ink-2 instead of 50% opacity so
 * they pass contrast. Used for Public log / Private log and
 * Assigned to them / Reported by them (with counts).
 * Pair each tab with <TabPanel id={…} tabsId={…}>.
 * ───────────────────────────────────────────────────────── */

export type TabItem<T extends string> = { value: T; label: ReactNode; count?: number | string };

export function Tabs<T extends string>({
  id,
  items,
  value,
  onChange,
  label,
  className,
}: {
  /** Prefix for tab/panel ids. */
  id: string;
  items: TabItem<T>[];
  value: T;
  onChange: (value: T) => void;
  label: string;
  className?: string;
}) {
  const index = Math.max(0, items.findIndex((t) => t.value === value));
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    let next = -1;
    if (e.key === "ArrowRight") next = (index + 1) % items.length;
    else if (e.key === "ArrowLeft") next = (index - 1 + items.length) % items.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = items.length - 1;
    if (next < 0) return;
    e.preventDefault();
    onChange(items[next]!.value);
    document.getElementById(`${id}-tab-${items[next]!.value}`)?.focus();
  };
  return (
    <div role="tablist" aria-label={label} onKeyDown={onKey} className={cn("flex items-center gap-0.5", className)}>
      {items.map((t, i) => {
        const selected = i === index;
        return (
          <button
            key={t.value}
            id={`${id}-tab-${t.value}`}
            type="button"
            role="tab"
            aria-selected={selected}
            aria-controls={`${id}-panel-${t.value}`}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(t.value)}
            className={cn(
              "inline-flex h-7 items-center gap-1.5 rounded-[7px] px-2.5 text-[13px] transition-[background-color,color] duration-100",
              selected ? "bg-hover-2 font-medium text-ink" : "text-ink-2 hover:bg-hover hover:text-ink",
            )}
          >
            {t.label}
            {t.count != null ? <span className="text-[11.5px] tabular-nums text-ink-2">{t.count}</span> : null}
          </button>
        );
      })}
    </div>
  );
}

export function TabPanel({
  tabsId,
  value,
  children,
  className,
}: {
  tabsId: string;
  value: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div role="tabpanel" id={`${tabsId}-panel-${value}`} aria-labelledby={`${tabsId}-tab-${value}`} className={className}>
      {children}
    </div>
  );
}
