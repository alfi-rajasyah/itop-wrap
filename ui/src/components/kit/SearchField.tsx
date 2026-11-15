import { forwardRef, type KeyboardEvent, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { SearchIcon, XIcon } from "./icons";
import { Spinner } from "./Spinner";

/* ─────────────────────────────────────────────────────────
 * SEARCH FIELD — Search's input row (magnifier, 13px input, a clear
 * button that fades in) as a standalone field for toolbars and the
 * menu bar panel. Esc clears (KEY-9); Esc on an empty field is left
 * to the page.
 * ───────────────────────────────────────────────────────── */

export const SearchField = forwardRef<
  HTMLInputElement,
  {
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    /** Accessible name; defaults to the placeholder. */
    label?: string;
    busy?: boolean;
    /** Trailing slot (e.g. a ⌘F hint). */
    trailing?: ReactNode;
    autoFocus?: boolean;
    onKeyDown?: (event: KeyboardEvent<HTMLInputElement>) => void;
    className?: string;
  }
>(function SearchField({ value, onChange, placeholder = "Search", label, busy, trailing, autoFocus, onKeyDown, className }, ref) {
  return (
    <div
      className={cn(
        "group flex h-8 min-w-0 items-center gap-2 rounded-control bg-field px-2.5 shadow-hairline transition-[box-shadow,background-color] duration-150",
        "hover:bg-hover-2 focus-within:bg-field focus-within:shadow-[0_0_0_1.5px_var(--ink-2)]",
        "has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-[var(--focus)]",
        className,
      )}
    >
      <SearchIcon className="size-3.5 shrink-0 text-ink-2" strokeWidth={2} />
      <input
        ref={ref}
        type="search"
        value={value}
        autoFocus={autoFocus}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          onKeyDown?.(e);
          if (e.defaultPrevented) return;
          if (e.key === "Escape" && value) {
            e.preventDefault();
            e.stopPropagation();
            onChange("");
          }
        }}
        placeholder={placeholder}
        aria-label={label ?? placeholder}
        spellCheck={false}
        autoComplete="off"
        className="min-w-0 flex-1 bg-transparent text-[13px] text-ink outline-none placeholder:text-ink-2/80 [&::-webkit-search-cancel-button]:hidden"
      />
      {busy ? <Spinner size={12} className="text-ink-2" /> : null}
      {value ? (
        <button
          aria-label="Clear search"
          type="button"
          onClick={() => onChange("")}
          className="-mr-1 flex size-5 shrink-0 items-center justify-center rounded-full text-ink-2 transition-colors duration-100 hover:bg-line-strong hover:text-ink"
          style={{ animation: "fade-in 150ms ease-out both" }}
        >
          <XIcon className="size-3" strokeWidth={2.2} />
        </button>
      ) : (
        trailing
      )}
    </div>
  );
});
