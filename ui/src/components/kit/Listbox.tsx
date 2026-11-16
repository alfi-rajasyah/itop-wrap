import {
  createContext,
  useContext,
  useEffect,
  useId,
  useRef,
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { cn } from "@/lib/utils";
import { scrollIntoNearest } from "./scroll";

/* ─────────────────────────────────────────────────────────
 * LISTBOX — the keyboard model of every master list (ticket lists,
 * CMDB results, orgs, people). One tab stop; ↑/↓ (and Home/End)
 * move the selection and the detail follows (KEY-9); ↩ calls
 * onOpen. The selected row is announced through
 * aria-activedescendant. Rows are any component that calls
 * useListboxOption(id); sections group them under a header.
 * ───────────────────────────────────────────────────────── */

type Ctx = { listId: string; selectedId: string | null | undefined; select: (id: string) => void };
const ListboxContext = createContext<Ctx | null>(null);

const optionDomId = (listId: string, id: string) => `${listId}-opt-${id.replace(/[^A-Za-z0-9_-]/g, "_")}`;

export function Listbox({
  label,
  selectedId,
  onSelect,
  onOpen,
  onKeyDown,
  children,
  className,
}: {
  /** Accessible name ("My tickets"). */
  label: string;
  selectedId: string | null | undefined;
  onSelect: (id: string) => void;
  /** ↩ on the selected row. */
  onOpen?: (id: string) => void;
  /** Extra shortcuts (e.g. ⌘C copies the selected ref). Runs first; preventDefault to stop ours. */
  onKeyDown?: (event: KeyboardEvent<HTMLDivElement>) => void;
  children: ReactNode;
  className?: string;
}) {
  const listId = useId();
  const ref = useRef<HTMLDivElement>(null);

  const ids = () =>
    Array.from(ref.current?.querySelectorAll<HTMLElement>("[role=option][data-option-id]") ?? []).map((el) => el.dataset.optionId!);

  // keep the selected row in view when selection changes from outside or by keyboard
  useEffect(() => {
    if (!selectedId) return;
    scrollIntoNearest(document.getElementById(optionDomId(listId, selectedId)));
  }, [selectedId, listId]);

  const handleKey = (e: KeyboardEvent<HTMLDivElement>) => {
    onKeyDown?.(e);
    if (e.defaultPrevented) return;
    if (e.target !== e.currentTarget) return; // keys inside a row's own controls
    const list = ids();
    if (!list.length) return;
    const i = selectedId ? list.indexOf(selectedId) : -1;
    let next: string | undefined;
    if (e.key === "ArrowDown") next = list[Math.min(list.length - 1, i + 1)];
    else if (e.key === "ArrowUp") next = list[i < 0 ? 0 : Math.max(0, i - 1)];
    else if (e.key === "Home") next = list[0];
    else if (e.key === "End") next = list[list.length - 1];
    else if (e.key === "Enter" && selectedId && onOpen) {
      e.preventDefault();
      onOpen(selectedId);
      return;
    }
    if (next !== undefined) {
      e.preventDefault();
      if (next !== selectedId) onSelect(next);
    }
  };

  return (
    <ListboxContext.Provider value={{ listId, selectedId, select: onSelect }}>
      <div
        ref={ref}
        role="listbox"
        aria-label={label}
        tabIndex={0}
        aria-activedescendant={selectedId ? optionDomId(listId, selectedId) : undefined}
        onKeyDown={handleKey}
        className={cn("group/listbox flex flex-col outline-none", className)}
      >
        {children}
      </div>
    </ListboxContext.Provider>
  );
}

/** Props for a row inside a Listbox. Click selects; the row isn't itself a tab stop. */
export function useListboxOption(id: string) {
  const ctx = useContext(ListboxContext);
  const selected = ctx?.selectedId === id;
  return {
    selected,
    props: {
      id: ctx ? optionDomId(ctx.listId, id) : undefined,
      role: "option" as const,
      "aria-selected": selected,
      "data-option-id": id,
      onMouseDown: (e: ReactMouseEvent<HTMLElement>) => {
        // keep keyboard focus on the listbox when a row is clicked
        const lb = (e.currentTarget as HTMLElement).closest<HTMLElement>("[role=listbox]");
        if (lb && !(e.target as HTMLElement).closest("button,a,input")) {
          e.preventDefault();
          lb.focus({ preventScroll: true });
        }
      },
      onClick: (e: ReactMouseEvent<HTMLElement>) => {
        if ((e.target as HTMLElement).closest("[data-row-action]")) return;
        ctx?.select(id);
      },
    },
  };
}

/** Group header: title left, the group's whole count right (LIST-13). Sticky in its scroller. */
export function SectionHeader({
  id,
  title,
  count,
  sticky = true,
  className,
}: {
  id?: string;
  title: ReactNode;
  count?: number | string;
  sticky?: boolean;
  className?: string;
}) {
  return (
    <div
      id={id}
      className={cn(
        "z-10 flex h-7 items-center justify-between gap-2 px-3 text-[11.5px] font-semibold text-ink-2",
        sticky && "sticky top-0 bg-[color-mix(in_srgb,var(--app-bg)_88%,transparent)] backdrop-blur-[6px]",
        className,
      )}
    >
      <span className="truncate">{title}</span>
      {count != null ? <span className="tabular-nums">{count}</span> : null}
    </div>
  );
}

/** role="group" of options under a SectionHeader. */
export function ListSection({
  title,
  count,
  children,
  className,
}: {
  title: ReactNode;
  count?: number | string;
  children: ReactNode;
  className?: string;
}) {
  const hid = useId();
  return (
    <div role="group" aria-labelledby={hid} className={cn("flex flex-col pb-1.5", className)}>
      <SectionHeader id={hid} title={title} count={count} />
      <div className="flex flex-col gap-px px-1.5">{children}</div>
    </div>
  );
}
