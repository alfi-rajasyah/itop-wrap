import type { ReactNode } from "react";
import GlideMenu from "@/components/primitives/GlideMenu";
import { cn } from "@/lib/utils";
import { NeedsYouDot } from "./Badges";
import { Button } from "./Button";
import { NewTicketIcon, OpenWindowIcon, RefreshIcon, StopIcon, StopwatchIcon } from "./icons";
import { IconButton } from "./IconButton";
import { Duration, Ref } from "./Ref";
import { SearchField } from "./SearchField";
import { formatCounter, useElapsedSeconds } from "./time";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * MENU BAR PANEL pieces (MB-7…16), 360px wide. Rows are Task Rows'
 * List rows compressed to one line (ref, title, time, a 26px timer
 * slot) on a GlideMenu hover layer. The running ticket's row is
 * tinted and its time is a bold live HH:MM:SS: no colour, so red
 * keeps its single meaning (only the Needs-you dots).
 * ───────────────────────────────────────────────────────── */

export function MenuBarPanel({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cn("flex w-[360px] flex-col overflow-hidden bg-surface text-ink", className)}>
      {children}
    </div>
  );
}

export function MenuBarTopRow({
  query,
  onQueryChange,
  onNewTicket,
  newTicketDisabled,
}: {
  query: string;
  onQueryChange: (q: string) => void;
  onNewTicket: () => void;
  newTicketDisabled?: boolean;
}) {
  return (
    <div className="flex items-center gap-2 border-b border-line p-2">
      <SearchField value={query} onChange={onQueryChange} placeholder="Search my tickets" className="flex-1" />
      <Button size="sm" icon={<NewTicketIcon />} onClick={onNewTicket} disabled={newTicketDisabled} tooltip="New ticket (⌘N)" aria-keyshortcuts="Meta+N">
        New ticket
      </Button>
    </div>
  );
}

/** "Escalated TTR  3" / "Running". */
export function MenuBarSection({ title, count, children }: { title: string; count?: number; children: ReactNode }) {
  return (
    <section aria-label={title} className="flex flex-col">
      <h3 className="flex h-[30px] items-end gap-2 px-3 pb-1.5 text-[11.5px] font-semibold text-ink-2">
        <span className="whitespace-pre">{title}</span>
        {count != null ? <span className="tabular-nums">{count}</span> : null}
      </h3>
      <GlideMenu rowSelector="[data-mb-row]" highlightClassName="inset-x-1.5 rounded-[8px] bg-hover" className="flex flex-col px-1.5">
        {children}
      </GlideMenu>
    </section>
  );
}

export type MenuBarTimer = "start" | "stop" | "none";

export function MenuBarTicketRow({
  ticketRef,
  title,
  needsYou = false,
  total,
  runningSince,
  runningLabel,
  timer = "none",
  onToggleTimer,
  timerDisabled,
  timerTooltip,
  timerBusy,
  onOpen,
}: {
  ticketRef: string;
  title: string;
  needsYou?: boolean;
  /** "1h 20m", shown when not running. */
  total?: string;
  /** Epoch ms: this row is the running timer. */
  runningSince?: number | null;
  /** The live HH:MM:SS made elsewhere (Rust's `timer.running.counter`); marks the row as running. Wins over runningSince. */
  runningLabel?: string | null;
  timer?: MenuBarTimer;
  onToggleTimer?: () => void;
  timerDisabled?: boolean;
  timerTooltip?: string;
  timerBusy?: boolean;
  onOpen?: () => void;
}) {
  const elapsed = useElapsedSeconds(runningLabel != null ? null : runningSince);
  const counter = runningLabel ?? (elapsed !== undefined ? formatCounter(elapsed) : undefined);
  const running = counter !== undefined;
  return (
    <div
      data-mb-row
      className={cn("relative z-10 flex h-12 items-center gap-2 rounded-[8px] pl-1.5 pr-1", running && "bg-field")}
    >
      <Tooltip content={`Open ${ticketRef} in iTop Wrap`} describe={false} className="min-w-0 flex-1">
        <button type="button" onClick={onOpen} className="flex min-w-0 flex-1 flex-col items-start rounded-[6px] px-1 py-0.5 text-left">
          <span className="flex items-center gap-1.5">
            {needsYou ? <NeedsYouDot size={7} /> : null}
            <Ref className="text-[11px]">{ticketRef}</Ref>
          </span>
          <span className="w-full truncate text-[12.5px] font-medium text-ink">{title}</span>
        </button>
      </Tooltip>
      {running ? (
        <Duration running className="text-[12px]">
          {counter}
        </Duration>
      ) : total ? (
        <Duration>{total}</Duration>
      ) : null}
      {timer === "stop" ? (
        <IconButton
          label={`Stop timer on ${ticketRef}`}
          tooltip={timerTooltip ?? "Stop the timer and save its time"}
          icon={<StopIcon />}
          size="sm"
          className="text-ink"
          onClick={onToggleTimer}
          disabled={timerDisabled}
          busy={timerBusy}
        />
      ) : timer === "start" ? (
        <IconButton
          label={`Start timer on ${ticketRef}`}
          tooltip={timerTooltip ?? "Start timer"}
          icon={<StopwatchIcon />}
          size="sm"
          onClick={onToggleTimer}
          disabled={timerDisabled}
          busy={timerBusy}
        />
      ) : (
        <span aria-hidden className="w-6.5 shrink-0" />
      )}
    </div>
  );
}

export function MenuBarFooter({
  onRefresh,
  refreshing = false,
  updatedLabel,
  onOpenApp,
}: {
  onRefresh: () => void;
  refreshing?: boolean;
  /** "Updated 2 minutes ago". */
  updatedLabel?: string;
  onOpenApp: () => void;
}) {
  return (
    <div className="flex h-10 items-center gap-1 border-t border-line px-1.5">
      <IconButton
        label="Refresh"
        tooltip="Refresh (⌘R)"
        icon={<RefreshIcon />}
        size="md"
        disabled={refreshing}
        onClick={onRefresh}
        aria-keyshortcuts="Meta+R"
      />
      <span className="min-w-0 flex-1 truncate text-center text-[11.5px] text-ink-2">{updatedLabel}</span>
      <IconButton label="Open iTop Wrap" tooltip="Open iTop Wrap (⌘O)" icon={<OpenWindowIcon />} size="md" onClick={onOpenApp} aria-keyshortcuts="Meta+O" />
    </div>
  );
}
