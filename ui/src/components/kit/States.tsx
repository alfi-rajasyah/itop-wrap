import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Button } from "./Button";
import { ErrorIcon, InfoIcon, XIcon } from "./icons";
import { IconButton } from "./IconButton";

/* ─────────────────────────────────────────────────────────
 * STATES — loading, empty, error, and the inline notice banner.
 *  - LoadingState: Loading State's pixel-grid loader ("Drive"
 *    chevron wavefront), without the shimmering label or the
 *    elapsed timer: a triage tool gets a calm, readable label.
 *    Reduced motion freezes the grid.
 *  - EmptyState: Search's empty state (icon in an inset tile, a
 *    13px title, a quiet hint).
 *  - ErrorState: the same frame with iTop's message and Retry.
 *  - NoticeBanner: one line above a list (LIST-69, TMR-20): an icon,
 *    the text, optional Retry, and a × to dismiss. Errors are red
 *    text on a faint red tint; info is neutral.
 * ───────────────────────────────────────────────────────── */

const chevron = Array.from({ length: 9 }, (_, i) => {
  const r = Math.floor(i / 3);
  const c = i % 3;
  return (c + Math.abs(r - 1)) * 110;
});

export function LoaderGrid({ className }: { className?: string }) {
  return (
    <span aria-hidden className={cn("grid shrink-0 grid-cols-[repeat(3,4px)] gap-[1.5px]", className)}>
      {chevron.map((delay, index) => (
        <span
          key={index}
          className="size-[4px] rounded-[1px] bg-ink"
          style={{ opacity: 0.15, animation: `pixel-on 1100ms ease-in-out ${delay}ms infinite` }}
        />
      ))}
    </span>
  );
}

export function LoadingState({ label, className }: { label: string; className?: string }) {
  return (
    <div role="status" className={cn("flex h-full items-center justify-center gap-2.5 px-4 py-10", className)}>
      <LoaderGrid />
      <span className="text-[13px] text-ink-2">{label}</span>
    </div>
  );
}

export function EmptyState({
  title,
  hint,
  icon,
  action,
  className,
}: {
  title: ReactNode;
  hint?: ReactNode;
  icon?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex h-full flex-col items-center justify-center gap-1 px-6 py-10 text-center", className)}>
      {icon ? (
        <span className="mb-1.5 flex size-8 items-center justify-center rounded-control bg-inset text-ink-2 shadow-hairline [&_svg]:size-4">
          {icon}
        </span>
      ) : null}
      <p className="max-w-[34ch] text-[13px] font-medium text-ink">{title}</p>
      {hint ? <p className="max-w-[40ch] text-[12.5px] text-ink-2">{hint}</p> : null}
      {action ? <div className="mt-2.5">{action}</div> : null}
    </div>
  );
}

export function ErrorState({
  message,
  onRetry,
  retryLabel = "Retry",
  retrying = false,
  className,
}: {
  message: ReactNode;
  onRetry?: () => void;
  retryLabel?: string;
  retrying?: boolean;
  className?: string;
}) {
  return (
    <div role="alert" className={cn("flex h-full flex-col items-center justify-center gap-3 px-6 py-10 text-center", className)}>
      <span className="flex size-8 items-center justify-center rounded-control bg-danger-tint text-danger">
        <ErrorIcon className="size-4" />
      </span>
      <p className="max-w-[44ch] text-[13px] leading-snug text-ink">{message}</p>
      {onRetry ? (
        <Button size="sm" onClick={onRetry} busy={retrying}>
          {retryLabel}
        </Button>
      ) : null}
    </div>
  );
}

export function NoticeBanner({
  tone = "info",
  icon,
  children,
  onRetry,
  retryLabel = "Retry",
  retryDisabled,
  onDismiss,
  dismissLabel = "Dismiss",
  className,
}: {
  tone?: "info" | "error";
  /** Leading icon; defaults to ⓘ / a warning circle. */
  icon?: ReactNode;
  children: ReactNode;
  onRetry?: () => void;
  retryLabel?: string;
  retryDisabled?: boolean;
  onDismiss?: () => void;
  /** Accessible name of the ×, e.g. "Discard this time" (TMR-11). */
  dismissLabel?: string;
  className?: string;
}) {
  const error = tone === "error";
  return (
    <div
      role={error ? "alert" : "status"}
      className={cn(
        "flex min-h-8 items-start gap-2 rounded-control px-2.5 py-1.5 text-[12.5px] leading-[18px]",
        error ? "bg-danger-tint text-danger" : "bg-inset text-ink shadow-hairline",
        className,
      )}
      style={{ animation: "fade-in 150ms ease-out both" }}
    >
      <span className={cn("mt-px flex shrink-0 [&_svg]:size-4", error ? "text-danger" : "text-ink-2")}>
        {icon ?? (error ? <ErrorIcon /> : <InfoIcon />)}
      </span>
      <p className="min-w-0 flex-1 py-px">{children}</p>
      {onRetry ? (
        <Button size="xs" variant="secondary" onClick={onRetry} disabled={retryDisabled} className="-my-0.5 h-6 shrink-0 text-ink">
          {retryLabel}
        </Button>
      ) : null}
      {onDismiss ? (
        <IconButton
          label={dismissLabel}
          icon={<XIcon strokeWidth={2} />}
          size="xs"
          onClick={onDismiss}
          className={cn("-my-px -mr-1 shrink-0", error && "text-danger hover:text-danger")}
        />
      ) : null}
    </div>
  );
}
