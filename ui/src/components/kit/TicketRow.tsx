import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { NeedsYouDot, PriorityMark, StatusPill } from "./Badges";
import { StopwatchIcon } from "./icons";
import { useListboxOption } from "./Listbox";
import { Duration, Ref } from "./Ref";
import { formatClock, useElapsedSeconds } from "./time";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * TICKET ROW — one ticket in a master list (LIST-52…57). Built on
 * Task Rows' List variant (13px medium label, quiet tabular meta,
 * hover:bg-inset), re-laid as three lines and set as a Sidebar-Nav
 * style rounded row with the selected tint:
 *
 *   ● R-000412 !                              3d
 *   VPN drops every 20 minutes on the        (title, 2 lines)
 *   Ada Lovelace · Org A        ⏱ 0:12  [assignee]
 *
 * The running timer shows as icon + bold live h:mm, never colour.
 * Hover actions (e.g. start timer) appear on hover or focus within.
 * Must be rendered inside a <Listbox>.
 * ───────────────────────────────────────────────────────── */

export type TicketRowProps = {
  id: string;
  ticketRef: string;
  title: string;
  priority?: number;
  needsYou?: boolean;
  /** "3d", "5h", "now". */
  age?: string;
  /** Tooltip for the age: "Opened 12 Nov 2026 at 09:14". */
  ageTitle?: string;
  caller?: string;
  org?: string;
  /** Only where the group header doesn't already say it. */
  status?: string;
  /** "1h 20m" (hidden while running). */
  total?: string;
  /** Epoch ms the timer started on this ticket; shows the live counter. */
  runningSince?: number | null;
  /** The live h:mm made elsewhere (Rust's `timer.running.clock`); marks the row as running. Wins over runningSince. */
  runningLabel?: string | null;
  /** Assignee / "Assign to me" pill, right-aligned on line 3. */
  assignee?: ReactNode;
  /** Buttons revealed on hover/focus, top-right. Mark each with data-row-action. */
  actions?: ReactNode;
  className?: string;
};

export function TicketRow({
  id,
  ticketRef,
  title,
  priority = 4,
  needsYou = false,
  age,
  ageTitle,
  caller,
  org,
  status,
  total,
  runningSince,
  runningLabel,
  assignee,
  actions,
  className,
}: TicketRowProps) {
  const { selected, props } = useListboxOption(id);
  const elapsed = useElapsedSeconds(runningLabel != null ? null : runningSince);
  const clock = runningLabel ?? (elapsed !== undefined ? formatClock(elapsed) : undefined);
  const running = clock !== undefined;
  const who = [caller, org].filter(Boolean).join(" · ");

  return (
    <div
      {...props}
      className={cn(
        "group/row relative flex cursor-default flex-col gap-0.5 rounded-[8px] px-2.5 py-2 transition-colors duration-100",
        selected ? "bg-hover-2" : "hover:bg-hover",
        selected && "group-focus-visible/listbox:shadow-[inset_0_0_0_2px_var(--focus)]",
        className,
      )}
    >
      {/* line 1 */}
      <div className="flex h-4 items-center gap-1.5">
        {needsYou ? <NeedsYouDot /> : null}
        <Ref>{ticketRef}</Ref>
        <PriorityMark priority={priority} />
        {status ? <StatusPill className="ml-0.5 h-4 px-1 text-[10.5px]">{status}</StatusPill> : null}
        <span className="flex-1" />
        {age ? (
          <Tooltip content={ageTitle} describe={false}>
            <span className="text-[11.5px] tabular-nums text-ink-2">{age}</span>
          </Tooltip>
        ) : null}
      </div>
      {/* line 2 */}
      <p className="line-clamp-2 text-[13px] font-medium leading-[1.35] text-ink">{title}</p>
      {/* line 3 */}
      {who || running || total || assignee ? (
        <div className="flex min-h-5 items-center gap-2">
          <span className="min-w-0 flex-1 truncate text-[12px] text-ink-2">{who}</span>
          {running ? (
            <Tooltip content="Timer running" describe={false}>
              <span className="inline-flex shrink-0 items-center gap-1 text-ink" role="img" aria-label={`Timer running, ${clock}`}>
                <StopwatchIcon className="size-3.5" strokeWidth={2} />
                <Duration running>{clock}</Duration>
              </span>
            </Tooltip>
          ) : total ? (
            <Tooltip content="Time spent" describe={false}>
              <span className="shrink-0" aria-label={`${total} spent`}>
                <Duration>{total}</Duration>
              </span>
            </Tooltip>
          ) : null}
          {assignee ? <span className="flex min-w-0 max-w-[45%] shrink-0 justify-end">{assignee}</span> : null}
        </div>
      ) : null}
      {actions ? (
        <div
          data-row-action
          className={cn(
            "absolute right-1.5 top-1.5 flex items-center gap-0.5 rounded-[7px] bg-surface p-0.5 shadow-raised",
            "opacity-0 transition-opacity duration-100 group-hover/row:opacity-100 focus-within:opacity-100",
          )}
        >
          {actions}
        </div>
      ) : null}
    </div>
  );
}

/** Compact row for CMDB results, orgs and people lists: leading slot, title, subtitle, trailing. */
export function SimpleRow({
  id,
  title,
  subtitle,
  leading,
  trailing,
  mono = false,
  className,
}: {
  id: string;
  title: ReactNode;
  subtitle?: ReactNode;
  leading?: ReactNode;
  trailing?: ReactNode;
  /** CI names use the mono motif (CMDB-5). */
  mono?: boolean;
  className?: string;
}) {
  const { selected, props } = useListboxOption(id);
  return (
    <div
      {...props}
      className={cn(
        "flex min-h-11 cursor-default items-center gap-2.5 rounded-[8px] px-2.5 py-1.5 transition-colors duration-100",
        selected ? "bg-hover-2" : "hover:bg-hover",
        selected && "group-focus-visible/listbox:shadow-[inset_0_0_0_2px_var(--focus)]",
        className,
      )}
    >
      {leading}
      <div className="min-w-0 flex-1">
        <p className={cn("truncate text-ink", mono ? "mono-figures text-[12.5px]" : "text-[13px] font-medium")}>{title}</p>
        {subtitle ? <p className="truncate text-[12px] text-ink-2">{subtitle}</p> : null}
      </div>
      {trailing ? <span className="shrink-0 text-[12px] tabular-nums text-ink-2">{trailing}</span> : null}
    </div>
  );
}
