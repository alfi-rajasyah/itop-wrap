import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { cn } from "@/lib/utils";
import { CheckIcon, ChevronUpDownIcon, SearchIcon } from "./icons";
import { Popover } from "./Popover";
import { Spinner } from "./Spinner";

/* ─────────────────────────────────────────────────────────
 * DROPDOWN FIELD — the one picker used everywhere (DESIGN.md,
 * KEY-11…17). The field is Beautiful UI's inset control (Fine-tune
 * Card's select / Search's input row); the list is Search's results
 * card with the GlideMenu-style grey highlight driven by the keyboard.
 *
 *  - 32px field, up/down chevron, border darkens while open/focused.
 *  - List below (above when it doesn't fit), ≤260px, 26px rows,
 *    check mark on the selected item.
 *  - Only one dropdown open at a time; outside click, Tab, Esc or a
 *    pick closes it. Esc on an open list doesn't reach the sheet.
 *  - Short lists: the field is a button-like combobox; ↓/↑ open and
 *    move, Return/Space pick (or open), a letter jumps.
 *  - Searchable lists: a text field; typing filters (every word must
 *    be in the label), highlights the first match and opens; ↓/↑
 *    move; Return picks (or opens); Esc closes and drops the text.
 *  - role="combobox" + aria-activedescendant on the focused field.
 * ───────────────────────────────────────────────────────── */

export type DropdownItem = { value: string; label: string };

const OPEN_EVENT = "kit-dropdown-open";

export type DropdownFieldProps = {
  items: DropdownItem[];
  value: string | null | undefined;
  onChange: (value: string) => void;
  /** Shown when nothing is selected: "Choose…", "Loading teams…", "Choose a team first". */
  placeholder?: string;
  searchable?: boolean;
  /** Placeholder of the search text ("Search organizations"). */
  searchPrompt?: string;
  /** Empty filter result ("No caller in Org A matches"). Default: Nothing matches “q”. */
  noMatchText?: string;
  loading?: boolean;
  disabled?: boolean;
  invalid?: boolean;
  id?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  "aria-required"?: boolean;
  className?: string;
};

function matches(label: string, query: string) {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const hay = label.toLowerCase();
  return words.every((w) => hay.includes(w));
}

export function DropdownField({
  items,
  value,
  onChange,
  placeholder = "Choose…",
  searchable = false,
  searchPrompt,
  noMatchText,
  loading = false,
  disabled = false,
  invalid = false,
  id,
  className,
  ...aria
}: DropdownFieldProps) {
  const autoId = useId();
  const fieldId = id ?? `dd-${autoId}`;
  const listId = `${fieldId}-list`;
  const fieldRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const [openState, setOpen] = useState(false);
  const [query, setQuery] = useState<string | null>(null);
  const [active, setActive] = useState(0);

  const selected = items.find((i) => i.value === value) ?? null;
  const shown = useMemo(
    () => (searchable && query ? items.filter((i) => matches(i.label, query)) : items),
    [items, query, searchable],
  );
  const canOpen = !disabled && items.length > 0;
  // a list that becomes disabled or empty closes by itself
  const open = openState && canOpen;

  // one dropdown open at a time
  useEffect(() => {
    const onOther = (e: Event) => {
      if ((e as CustomEvent<string>).detail !== fieldId) setOpen(false);
    };
    window.addEventListener(OPEN_EVENT, onOther);
    return () => window.removeEventListener(OPEN_EVENT, onOther);
  }, [fieldId]);

  // keep the highlighted row visible
  useEffect(() => {
    if (!open) return;
    const el = document.getElementById(`${listId}-${active}`);
    const box = listRef.current;
    if (el && box) {
      if (el.offsetTop < box.scrollTop) box.scrollTop = el.offsetTop - 4;
      else if (el.offsetTop + el.offsetHeight > box.scrollTop + box.clientHeight) box.scrollTop = el.offsetTop + el.offsetHeight - box.clientHeight + 4;
    }
  }, [active, open, listId]);

  const indexOfSelected = (list: DropdownItem[]) => Math.max(0, list.findIndex((i) => i.value === value));

  const doOpen = (highlight?: number) => {
    if (!canOpen) return;
    window.dispatchEvent(new CustomEvent(OPEN_EVENT, { detail: fieldId }));
    setActive(highlight ?? indexOfSelected(shown));
    setOpen(true);
  };
  const doClose = () => {
    setOpen(false);
    setQuery(null);
  };
  const pick = (item: DropdownItem | undefined) => {
    if (!item) return;
    onChange(item.value);
    doClose();
  };
  const move = (delta: number) => {
    if (!open) {
      doOpen();
      return;
    }
    if (!shown.length) return;
    setActive((a) => Math.min(shown.length - 1, Math.max(0, a + delta)));
  };

  const onKey = (e: KeyboardEvent<HTMLElement>) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(1);
        return;
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        return;
      case "Home":
        if (open && !searchable) {
          e.preventDefault();
          setActive(0);
        }
        return;
      case "End":
        if (open && !searchable) {
          e.preventDefault();
          setActive(Math.max(0, shown.length - 1));
        }
        return;
      case "Enter":
        if (e.metaKey || e.ctrlKey) return; // ⌘↩ belongs to the sheet
        e.preventDefault();
        if (open) pick(shown[active]);
        else doOpen();
        return;
      case " ":
        if (searchable) return; // a space is text
        e.preventDefault();
        if (open) pick(shown[active]);
        else doOpen();
        return;
      case "Escape":
        if (open) {
          e.preventDefault();
          e.stopPropagation();
          doClose();
        } else if (searchable && query) {
          e.preventDefault();
          e.stopPropagation();
          setQuery(null);
        }
        return;
      case "Tab":
        if (open) doClose();
        return;
      default:
        if (!searchable && e.key.length === 1 && /\S/.test(e.key) && !e.metaKey && !e.ctrlKey && !e.altKey) {
          const ch = e.key.toLowerCase();
          const i = items.findIndex((it) => it.label.toLowerCase().startsWith(ch));
          if (i >= 0) {
            if (open) setActive(i);
            else doOpen(i);
          }
        }
    }
  };

  const activeId = open && shown.length ? `${listId}-${active}` : undefined;
  const ariaCommon = {
    role: "combobox" as const,
    "aria-expanded": open,
    "aria-controls": open ? listId : undefined,
    "aria-haspopup": "listbox" as const,
    "aria-activedescendant": activeId,
    "aria-invalid": invalid || undefined,
    "aria-busy": loading || undefined,
    ...aria,
  };

  const display = selected?.label ?? "";

  return (
    <div className={cn("relative min-w-0", className)}>
      <div
        ref={fieldRef}
        onMouseDown={(e) => {
          if (disabled) return;
          // clicking the field (not the input text) toggles
          if (e.target === inputRef.current && open) return;
          if (e.target !== inputRef.current) e.preventDefault();
          (searchable ? inputRef.current : buttonRef.current)?.focus();
          if (open) doClose();
          else doOpen();
        }}
        className={cn(
          "flex h-8 w-full items-center gap-1.5 rounded-control bg-field px-2.5 text-[13px] shadow-hairline",
          "transition-[box-shadow,background-color] duration-150",
          disabled ? "cursor-default opacity-55" : "cursor-pointer hover:bg-hover-2",
          (open || invalid) && "shadow-[0_0_0_1.5px_var(--ink-2)]",
          invalid && "shadow-[0_0_0_1.5px_var(--danger)]",
          "has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-[var(--focus)]",
        )}
      >
        {searchable ? (
          <>
            <SearchIcon className="size-3.5 shrink-0 text-ink-2" strokeWidth={2} />
            <input
              ref={inputRef}
              id={fieldId}
              {...ariaCommon}
              aria-autocomplete="list"
              disabled={disabled}
              value={query ?? display}
              placeholder={selected ? undefined : open ? (searchPrompt ?? placeholder) : placeholder}
              onFocus={(e) => e.currentTarget.select()}
              onChange={(e) => {
                setQuery(e.target.value);
                setActive(0);
                if (!open && canOpen) {
                  window.dispatchEvent(new CustomEvent(OPEN_EVENT, { detail: fieldId }));
                  setOpen(true);
                }
              }}
              onKeyDown={onKey}
              onBlur={doClose}
              autoComplete="off"
              spellCheck={false}
              className="min-w-0 flex-1 cursor-text bg-transparent text-ink outline-none placeholder:text-ink-2 disabled:cursor-default"
            />
          </>
        ) : (
          <button
            ref={buttonRef}
            id={fieldId}
            type="button"
            {...ariaCommon}
            disabled={disabled}
            onKeyDown={onKey}
            onBlur={doClose}
            className={cn(
              "min-w-0 flex-1 truncate bg-transparent text-left outline-none disabled:cursor-default",
              selected ? "text-ink" : "text-ink-2",
            )}
          >
            {display || placeholder}
          </button>
        )}
        {loading ? <Spinner size={12} className="text-ink-2" /> : null}
        <ChevronUpDownIcon className="size-4 shrink-0 text-ink-2" />
      </div>

      <Popover
        anchorRef={fieldRef}
        open={open}
        onDismiss={doClose}
        matchWidth
        maxHeight={260}
        floatingRef={listRef}
        className="p-1"
        onMouseDown={(e) => e.preventDefault() /* keep focus in the field */}
      >
        <div id={listId} role="listbox" aria-labelledby={aria["aria-labelledby"]} aria-label={aria["aria-label"]}>
          {shown.length === 0 ? (
            <div className="px-2 py-1.5 text-[12.5px] text-ink-2">
              {noMatchText ?? `Nothing matches “${(query ?? "").trim()}”.`}
            </div>
          ) : (
            <div className="relative flex flex-col">
              {shown.map((item, i) => {
                const isSel = item.value === value;
                const isActive = i === active;
                return (
                  <div
                    key={item.value}
                    id={`${listId}-${i}`}
                    role="option"
                    aria-selected={isSel}
                    onMouseMove={() => i !== active && setActive(i)}
                    onClick={() => pick(item)}
                    className={cn(
                      "flex h-[26px] cursor-pointer items-center gap-2 rounded-[6px] px-2 text-[13px] text-ink transition-colors duration-75",
                      isActive && "bg-hover-2",
                    )}
                  >
                    <span className="min-w-0 flex-1 truncate">{item.label}</span>
                    <CheckIcon className={cn("size-3.5 shrink-0", isSel ? "text-ink" : "invisible")} strokeWidth={2.2} />
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </Popover>
    </div>
  );
}

/** A short-list dropdown (Impact, Urgency, Resolution code, Team…). Same component. */
export function Select(props: Omit<DropdownFieldProps, "searchable" | "searchPrompt" | "noMatchText">) {
  return <DropdownField {...props} searchable={false} />;
}
