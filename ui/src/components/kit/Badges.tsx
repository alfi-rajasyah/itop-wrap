import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * BADGES — status pill, Needs-you pill/dot, priority mark, count.
 * Shape from Filter Table's status pill and the ValuePill atom; the
 * colour rules are iTop Wrap's (DESIGN.md):
 *  - status is always written, on a neutral tint, never colour alone;
 *  - teal only for "needs you";
 *  - red only for priority 1.
 * ───────────────────────────────────────────────────────── */

/** Neutral status label ("Assigned", "Escalated TTR"). The text is the meaning. */
export function StatusPill({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex h-5 shrink-0 items-center whitespace-nowrap rounded-[6px] bg-field px-1.5 text-[11.5px] font-medium text-ink-2 shadow-hairline",
        className,
      )}
    >
      {children}
    </span>
  );
}

/** Teal "Needs you" capsule: detail header, and anywhere the words fit. */
export function NeedsYouPill({ className, label = "Needs you" }: { className?: string; label?: string }) {
  return (
    <span
      className={cn(
        "inline-flex h-5 shrink-0 items-center gap-1 whitespace-nowrap rounded-full bg-need-tint px-2 text-[11.5px] font-semibold text-need-ink",
        className,
      )}
      style={{ boxShadow: "0 0 0 1px color-mix(in srgb, var(--need) 30%, transparent)" }}
    >
      <span aria-hidden className="size-1.5 rounded-full bg-need" />
      {label}
    </span>
  );
}

/** The 8px teal dot on list rows (7px in the menu bar). Named "Needs you" for AT. */
export function NeedsYouDot({ size = 8, className }: { size?: number; className?: string }) {
  return (
    <Tooltip content="Needs you" describe={false}>
      <span
        role="img"
        aria-label="Needs you"
        className={cn("inline-block shrink-0 rounded-full bg-need", className)}
        style={{ width: size, height: size }}
      />
    </Tooltip>
  );
}

/** Priority mark (LIST-53): P1 a bold red "!", P2 a 6px grey dot, P3/P4 nothing. */
export function PriorityMark({ priority, className }: { priority: number; className?: string }) {
  if (priority === 1)
    return (
      <Tooltip content="Priority 1" describe={false}>
        <span role="img" aria-label="Priority 1" className={cn("inline-flex w-2 shrink-0 justify-center text-[13px] font-extrabold leading-none text-danger", className)}>
          !
        </span>
      </Tooltip>
    );
  if (priority === 2)
    return (
      <Tooltip content="Priority 2" describe={false}>
        <span role="img" aria-label="Priority 2" className={cn("inline-flex w-2 shrink-0 items-center justify-center", className)}>
          <span className="size-1.5 rounded-full bg-ink-3" />
        </span>
      </Tooltip>
    );
  return null;
}

/** Small count, e.g. a sidebar or tab count. `tone="need"` is the teal Needs-you capsule. */
export function CountBadge({
  count,
  tone = "neutral",
  className,
  label,
}: {
  count: number | string;
  tone?: "neutral" | "need";
  className?: string;
  /** Accessible text, e.g. "3 need you". */
  label?: string;
}) {
  return (
    <span
      aria-label={label}
      className={cn(
        "inline-flex h-[18px] min-w-[18px] shrink-0 items-center justify-center rounded-full px-1.5 text-[11px] font-semibold tabular-nums leading-none",
        tone === "need" ? "bg-need text-on-need" : "text-ink-2",
        className,
      )}
    >
      {count}
    </span>
  );
}
