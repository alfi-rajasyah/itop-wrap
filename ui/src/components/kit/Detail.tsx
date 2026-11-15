import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { IncidentIcon, LockIcon, PaperclipIcon, ServerIcon, UserRequestIcon } from "./icons";
import { Spinner } from "./Spinner";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * DETAIL PIECES
 *  - SectionHeading: "Description", "Attachments", "Relationships".
 *  - KeyValueGrid: CI fields / person info (CMDB-12, PPL-9), an
 *    adaptive grid of ≥160px columns, label over value.
 *  - DetailStrip + DetailItem: the tinted box of icon-labelled
 *    facts under a ticket's title (DET-25); wraps onto new lines.
 *  - ClassIcon: User request / Incident glyph with its name.
 *  - LogEntry: one case-log entry card (DET-37), rich content slot.
 *  - AttachmentChip, CIChip: Context Cards' source chips — inset
 *    pill, hairline ring, 12px label — with a paperclip / server.
 * ───────────────────────────────────────────────────────── */

export function SectionHeading({ children, trailing, className }: { children: ReactNode; trailing?: ReactNode; className?: string }) {
  return (
    <div className={cn("flex items-center gap-2", className)}>
      <h3 className="text-[12.5px] font-semibold text-ink">{children}</h3>
      {trailing ? <span className="text-[12px] tabular-nums text-ink-2">{trailing}</span> : null}
    </div>
  );
}

export type KeyValue = { key: string; label: ReactNode; value: ReactNode };

export function KeyValueGrid({ items, minColumn = 160, className }: { items: KeyValue[]; minColumn?: number; className?: string }) {
  return (
    <dl className={cn("grid gap-x-6 gap-y-3", className)} style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${minColumn}px, 1fr))` }}>
      {items.map((it) => (
        <div key={it.key} className="min-w-0">
          <dt className="text-[11.5px] text-ink-2">{it.label}</dt>
          <dd className="mt-0.5 break-words text-[13px] text-ink">{it.value}</dd>
        </div>
      ))}
    </dl>
  );
}

export function DetailStrip({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cn("flex flex-wrap items-center gap-x-4 gap-y-2 rounded-card bg-inset px-3 py-2.5 shadow-hairline", className)}>
      {children}
    </div>
  );
}

/** One fact in the strip: an icon that names the field (tooltip + a11y) and its value. */
export function DetailItem({
  icon,
  label,
  children,
  emptyText = "None",
  className,
}: {
  icon?: ReactNode;
  /** Field name: "Caller", "Organization", "Team", "Agent", "Time spent". */
  label: string;
  children?: ReactNode;
  emptyText?: string;
  className?: string;
}) {
  const empty = children == null || children === "";
  return (
    <Tooltip content={label} describe={false}>
      <span
        className={cn("inline-flex min-w-0 max-w-full items-center gap-1.5 text-[12.5px]", empty ? "text-ink-2" : "text-ink", className)}
        aria-label={typeof children === "string" || empty ? `${label}: ${empty ? "none" : children}` : undefined}
      >
        {icon ? <span className="flex shrink-0 text-ink-2 [&_svg]:size-[15px]">{icon}</span> : null}
        <span className="min-w-0 truncate">{empty ? emptyText : children}</span>
      </span>
    </Tooltip>
  );
}

export function ClassIcon({ kind, className }: { kind: "UserRequest" | "Incident"; className?: string }) {
  const name = kind === "Incident" ? "Incident" : "User request";
  return (
    <Tooltip content={name} describe={false}>
      <span role="img" aria-label={name} className={cn("inline-flex text-ink-2 [&_svg]:size-[15px]", className)}>
        {kind === "Incident" ? <IncidentIcon /> : <UserRequestIcon />}
      </span>
    </Tooltip>
  );
}

export function LogEntry({
  author,
  date,
  isPrivate = false,
  children,
  className,
}: {
  author?: string;
  date?: string;
  isPrivate?: boolean;
  /** The entry's rich text. */
  children: ReactNode;
  className?: string;
}) {
  const head = [author, date].filter(Boolean).join(" · ");
  return (
    <article className={cn("rounded-card bg-surface shadow-card", className)}>
      {head || isPrivate ? (
        <header className="flex items-center gap-1.5 border-b border-line px-3 py-2 text-[12px] text-ink-2">
          {isPrivate ? <LockIcon role="img" aria-label="Private" className="size-3.5 shrink-0" /> : null}
          {author ? <span className="font-medium text-ink">{author}</span> : null}
          {author && date ? <span aria-hidden>·</span> : null}
          {date ? <time className="tabular-nums">{date}</time> : null}
        </header>
      ) : null}
      <div className="px-3 py-2.5 text-[13px] leading-[1.55] text-ink [overflow-wrap:anywhere]">{children}</div>
    </article>
  );
}

const chipClass =
  "inline-flex h-6.5 max-w-full items-center gap-1.5 rounded-chip bg-field pl-1.5 pr-2 text-[12px] text-ink shadow-hairline transition-[background-color,transform] duration-100 hover:bg-hover-2 active:scale-[0.97] disabled:pointer-events-none disabled:opacity-50";

export function AttachmentChip({
  name,
  onClick,
  busy = false,
  disabled = false,
  className,
}: {
  name: string;
  onClick?: () => void;
  busy?: boolean;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <Tooltip content={`Download ${name}`} describe={false}>
      <button type="button" onClick={onClick} disabled={disabled || busy} aria-busy={busy || undefined} aria-label={`Download ${name}`} className={cn(chipClass, className)}>
        {busy ? <Spinner size={12} className="text-ink-2" /> : <PaperclipIcon className="size-3.5 shrink-0 text-ink-2" />}
        <span className="truncate">{name}</span>
      </button>
    </Tooltip>
  );
}

/**
 * A configuration item. `variant="chip"` (relationships, CMDB-14) or `"link"` (the
 * underlined mono label in a ticket's details strip, DET-28).
 */
export function CIChip({
  name,
  tooltip,
  onClick,
  variant = "chip",
  className,
}: {
  name: string;
  /** "Server: open web-01" / "Linked CI: open web-01 in Config items". */
  tooltip?: string;
  onClick?: () => void;
  variant?: "chip" | "link";
  className?: string;
}) {
  const btn =
    variant === "link" ? (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          "mono-figures inline-flex max-w-full items-center gap-1.5 rounded-[4px] text-[12px] text-ink underline decoration-line-strong underline-offset-[3px] hover:decoration-ink",
          className,
        )}
      >
        <ServerIcon className="size-[15px] shrink-0 text-ink-2" />
        <span className="truncate">{name}</span>
      </button>
    ) : (
      <button type="button" onClick={onClick} className={cn(chipClass, "mono-figures", className)}>
        <ServerIcon className="size-3.5 shrink-0 text-ink-2" />
        <span className="truncate">{name}</span>
      </button>
    );
  return tooltip ? (
    <Tooltip content={tooltip} describe>
      {btn}
    </Tooltip>
  ) : (
    btn
  );
}
