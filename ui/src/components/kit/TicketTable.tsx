import type { KeyboardEvent } from "react";
import { cn } from "@/lib/utils";
import { Assignee } from "./Avatar";
import { NeedsYouDot, PriorityMark } from "./Badges";
import { Ref } from "./Ref";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * TICKET TABLE — an organization's or a person's tickets (ORG-11,
 * PPL-14). Adapted from Filter Table: the same card, header row in
 * 12.5px ink-2 and hairline grid lines, with status groups as
 * full-width header rows. Each row opens its ticket (click or ↩).
 * ───────────────────────────────────────────────────────── */

export type TicketTableColumn = "ref" | "title" | "caller" | "assignee";

export type TicketTableRow = {
  id: string;
  ticketRef: string;
  title: string;
  priority?: number;
  needsYou?: boolean;
  caller?: string;
  /** "You", an agent's name; ignored when unassigned. */
  assigneeName?: string;
  unassigned?: boolean;
};

export type TicketTableGroup = { title: string; count: number; rows: TicketTableRow[] };

const COLUMN_LABEL: Record<TicketTableColumn, string> = { ref: "Ref", title: "Title", caller: "Caller", assignee: "Assignee" };
const COLUMN_WIDTH: Record<TicketTableColumn, string> = {
  ref: "106px",
  title: "minmax(0,1fr)",
  caller: "minmax(0,130px)",
  assignee: "minmax(0,140px)",
};

export function TicketTable({
  columns = ["ref", "title", "caller", "assignee"],
  groups,
  onOpen,
  label = "Tickets",
  className,
}: {
  columns?: TicketTableColumn[];
  groups: TicketTableGroup[];
  onOpen: (id: string) => void;
  label?: string;
  className?: string;
}) {
  const template = columns.map((c) => COLUMN_WIDTH[c]).join(" ");
  const onRowKey = (e: KeyboardEvent<HTMLDivElement>, id: string) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onOpen(id);
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const rows = Array.from(e.currentTarget.closest("[role=table]")?.querySelectorAll<HTMLElement>("[data-ticket-row]") ?? []);
      const i = rows.indexOf(e.currentTarget);
      rows[e.key === "ArrowDown" ? Math.min(rows.length - 1, i + 1) : Math.max(0, i - 1)]?.focus();
    }
  };

  return (
    <div role="table" aria-label={label} className={cn("overflow-hidden rounded-card bg-surface shadow-card", className)}>
      <div role="rowgroup">
        <div
          role="row"
          className="grid border-b border-[var(--grid-line)] text-[12px] font-medium text-ink-2"
          style={{ gridTemplateColumns: template }}
        >
          {columns.map((c, i) => (
            <span key={c} role="columnheader" className={cn("px-3 py-1.5", i < columns.length - 1 && "border-r border-[var(--grid-line)]")}>
              {COLUMN_LABEL[c]}
            </span>
          ))}
        </div>
      </div>
      {groups.map((g) => (
        <div role="rowgroup" key={g.title}>
          <div role="row" className="flex items-center justify-between border-b border-[var(--grid-line)] bg-inset px-3 py-1 text-[11.5px] font-semibold text-ink-2">
            <span role="rowheader">{g.title}</span>
            <span className="tabular-nums">{g.count}</span>
          </div>
          {g.rows.map((row) => (
            <Tooltip key={row.id} content={`Open ${row.ticketRef}`} describe={false} className="flex">
              <div
                role="row"
                tabIndex={0}
                data-ticket-row
                onClick={() => onOpen(row.id)}
                onKeyDown={(e) => onRowKey(e, row.id)}
                className="grid w-full cursor-pointer border-b border-[var(--grid-line)] text-[13px] transition-colors duration-100 hover:bg-hover focus-visible:-outline-offset-2"
                style={{ gridTemplateColumns: template }}
              >
                {columns.map((c, i) => (
                  <span
                    key={c}
                    role="cell"
                    className={cn("flex min-w-0 items-center gap-1.5 px-3 py-[7px]", i < columns.length - 1 && "border-r border-[var(--grid-line)]")}
                  >
                    {c === "ref" ? (
                      <>
                        {row.needsYou ? <NeedsYouDot size={7} /> : null}
                        <Ref>{row.ticketRef}</Ref>
                        {row.priority === 1 ? <PriorityMark priority={1} /> : null}
                      </>
                    ) : c === "title" ? (
                      <span className="truncate font-medium text-ink">{row.title}</span>
                    ) : c === "caller" ? (
                      <span className="truncate text-ink-2">{row.caller}</span>
                    ) : (
                      <Assignee name={row.assigneeName} unassigned={row.unassigned} />
                    )}
                  </span>
                ))}
              </div>
            </Tooltip>
          ))}
        </div>
      ))}
    </div>
  );
}
