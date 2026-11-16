import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { ChevronLeftIcon, ChevronRightIcon } from "./icons";
import { IconButton } from "./IconButton";
import { Spinner } from "./Spinner";

/* ─────────────────────────────────────────────────────────
 * PAGER + LIST FOOTER (LIST-49, LIST-68). Records Table's footer
 * grammar: quiet 12px tabular text between small icon buttons.
 * ───────────────────────────────────────────────────────── */

export function Pager({
  first,
  last,
  total,
  onPrevious,
  onNext,
  noun = { one: "ticket", other: "tickets" },
  className,
}: {
  /** 1-based index of the first item on the page. */
  first: number;
  last: number;
  total: number;
  onPrevious: () => void;
  onNext: () => void;
  noun?: { one: string; other: string };
  className?: string;
}) {
  if (total <= 0) return null;
  const single = first <= 1 && last >= total;
  if (single)
    return (
      <span className={cn("text-[12px] tabular-nums text-ink-2", className)}>
        {total} {total === 1 ? noun.one : noun.other}
      </span>
    );
  return (
    <nav aria-label="Pages" className={cn("flex items-center gap-0.5", className)}>
      <IconButton label="Previous page" icon={<ChevronLeftIcon strokeWidth={2} />} size="sm" disabled={first <= 1} onClick={onPrevious} />
      <span className="min-w-[88px] text-center text-[12px] tabular-nums text-ink-2" aria-live="polite">
        {first}–{last} of {total}
      </span>
      <IconButton label="Next page" icon={<ChevronRightIcon strokeWidth={2} />} size="sm" disabled={last >= total} onClick={onNext} />
    </nav>
  );
}

/** The bar under a list: pager on the left, refresh state and "Updated …" on the right. */
export function ListFooter({
  pager,
  refreshing = false,
  updatedLabel,
  className,
}: {
  pager?: ReactNode;
  refreshing?: boolean;
  /** "Updated just now", "Updated 3 min ago", "Loading", "Not updated yet". */
  updatedLabel?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex h-9 shrink-0 items-center gap-2 border-t border-line px-2.5", className)}>
      <div className="flex min-w-0 items-center">{pager}</div>
      <span className="flex-1" />
      {refreshing ? <Spinner size={12} className="text-ink-2" label="Refreshing" /> : null}
      {updatedLabel ? <span className="truncate text-[11.5px] text-ink-2">{updatedLabel}</span> : null}
    </div>
  );
}

/** Quiet line saying how the view differs from the default (LIST-42/43), up to 2 lines. */
export function ViewSummary({ children, className }: { children?: ReactNode; className?: string }) {
  if (!children) return null;
  return <p className={cn("line-clamp-2 text-[11.5px] leading-snug text-ink-2", className)}>{children}</p>;
}
