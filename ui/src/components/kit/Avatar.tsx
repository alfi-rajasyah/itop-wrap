import { cn } from "@/lib/utils";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * PEOPLE — initials avatar, unassigned mark, assignee chip and the
 * dashed "Assign to me" pill. Adapted from the EntityChip / Monogram
 * atoms: same disc + name in a soft pill, but a neutral disc (no
 * invented colours or photos, DESIGN.md R-23).
 * ───────────────────────────────────────────────────────── */

/** First letters of the first two words ("Agent Two" → "AT"). */
export function initialsOf(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  return words
    .slice(0, 2)
    .map((w) => w[0]!.toUpperCase())
    .join("");
}

export function InitialsAvatar({
  name,
  initials,
  size = 18,
  className,
}: {
  name: string;
  initials?: string;
  size?: number;
  className?: string;
}) {
  const text = initials ?? initialsOf(name);
  return (
    <span
      aria-hidden
      className={cn(
        "inline-flex shrink-0 select-none items-center justify-center rounded-full bg-hover-2 font-semibold leading-none text-ink-2 shadow-hairline",
        className,
      )}
      style={{ width: size, height: size, fontSize: Math.max(8, Math.round(size * 0.42)) }}
    >
      {text}
    </span>
  );
}

/** Dashed empty circle: "nobody owns this". */
export function UnassignedMark({ size = 18, className }: { size?: number; className?: string }) {
  return (
    <span
      aria-hidden
      className={cn("inline-block shrink-0 rounded-full border-[1.5px] border-dashed border-ink-3", className)}
      style={{ width: size, height: size }}
    />
  );
}

/**
 * The agent of a ticket: avatar + name ("You", "Agent 2"), or a dashed circle + bold
 * "Unassigned". `compact` shows only the avatar/mark (list rows), with the name as tooltip.
 */
export function Assignee({
  name,
  unassigned = false,
  compact = false,
  className,
}: {
  /** Display name: "You" for yourself, else the agent's name. Ignored when unassigned. */
  name?: string;
  unassigned?: boolean;
  compact?: boolean;
  className?: string;
}) {
  if (unassigned) {
    const body = (
      <span className={cn("inline-flex min-w-0 items-center gap-1.5", className)} aria-label={compact ? "Unassigned" : undefined} role={compact ? "img" : undefined}>
        <UnassignedMark size={compact ? 16 : 18} />
        {!compact && <span className="truncate text-[12px] font-semibold text-ink">Unassigned</span>}
      </span>
    );
    return compact ? <Tooltip content="Unassigned" describe={false}>{body}</Tooltip> : body;
  }
  const display = name ?? "";
  const body = (
    <span className={cn("inline-flex min-w-0 items-center gap-1.5", className)} aria-label={compact ? display : undefined} role={compact ? "img" : undefined}>
      <InitialsAvatar name={display} size={compact ? 16 : 18} />
      {!compact && <span className="truncate text-[12px] text-ink-2">{display}</span>}
    </span>
  );
  return compact ? <Tooltip content={display} describe={false}>{body}</Tooltip> : body;
}

/** Dashed "Assign to me" pill (LIST-57/58). */
export function AssignToMePill({
  ticketRef,
  onClick,
  disabled,
  className,
}: {
  ticketRef: string;
  onClick?: () => void;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <Tooltip content={`Assign ${ticketRef} to you`} describe={false}>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onClick?.();
        }}
        disabled={disabled}
        aria-label={`Assign ${ticketRef} to me`}
        className={cn(
          "inline-flex h-5 shrink-0 items-center gap-1 whitespace-nowrap rounded-full border border-dashed border-ink-3 px-2 text-[11.5px] font-medium text-ink",
          "transition-[background-color,border-color,transform] duration-150 hover:border-ink-2 hover:bg-hover-2 active:scale-[0.96] disabled:pointer-events-none disabled:opacity-50",
          className,
        )}
      >
        Assign to me
      </button>
    </Tooltip>
  );
}
