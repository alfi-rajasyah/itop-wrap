import { cn } from "@/lib/utils";
import { GlobeIcon, StopIcon, StopwatchIcon } from "./icons";
import { IconButton } from "./IconButton";
import { Duration } from "./Ref";
import { formatCounter, useElapsedSeconds } from "./time";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * TIMER BUTTON (TMR-19). Neutral, never teal: the running state is
 * the icon change (stopwatch → stop) plus a live bold HH:MM:SS.
 * Digits change in place with no animation. Same 26px header-control
 * chip as the More button.
 *
 *   idle     stopwatch icon            "Start timer"
 *   running  ■ 00:12:34 (bold mono)     "Stop timer"
 *   web      globe · Web stopwatch since 09:14 (a label, no button)
 * ───────────────────────────────────────────────────────── */

export type TimerButtonProps = {
  state: "idle" | "running" | "web";
  /** Epoch ms, for the live counter while running. */
  startedAt?: number | null;
  /** "09:14" / "12 Nov, 09:14" for the web stopwatch label. */
  webSince?: string;
  onToggle?: () => void;
  disabled?: boolean;
  /** Saving the previous entry. */
  busy?: boolean;
  /** Overrides the default tooltip (e.g. "Start timer: stops the one on R-000398 and saves its time (⌘T)"). */
  tooltip?: string;
  className?: string;
};

export function TimerButton({ state, startedAt, webSince, onToggle, disabled, busy, tooltip, className }: TimerButtonProps) {
  const elapsed = useElapsedSeconds(state === "running" ? startedAt ?? Date.now() : null);

  if (state === "web") {
    return (
      <Tooltip content="Your stopwatch in iTop's web UI is running on this ticket, so the app doesn't start its own">
        <span className={cn("inline-flex h-6.5 items-center gap-1.5 rounded-[7px] bg-hover-2 px-2 text-[12px] text-ink-2", className)}>
          <GlobeIcon className="size-3.5 shrink-0" />
          Web stopwatch since {webSince}
        </span>
      </Tooltip>
    );
  }

  if (state === "running") {
    return (
      <IconButton
        label="Stop timer"
        tooltip={tooltip ?? "Stop timer: saves its time (⌘T)"}
        icon={<StopIcon />}
        variant="field"
        size="sm"
        onClick={onToggle}
        disabled={disabled}
        busy={busy}
        aria-keyshortcuts="Meta+T"
        className={cn("pl-1.5 pr-2", className)}
      >
        <Duration running className="text-[12px]">
          {formatCounter(elapsed ?? 0)}
        </Duration>
      </IconButton>
    );
  }

  return (
    <IconButton
      label="Start timer"
      tooltip={tooltip ?? (disabled ? "Changes to iTop are switched off in this build" : "Start timer (⌘T)")}
      icon={<StopwatchIcon />}
      variant="field"
      size="sm"
      onClick={onToggle}
      disabled={disabled}
      busy={busy}
      aria-keyshortcuts="Meta+T"
      className={cn("min-w-[34px]", className)}
    />
  );
}
