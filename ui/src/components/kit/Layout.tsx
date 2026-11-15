import { useRef, useState, type KeyboardEvent, type PointerEvent, type ReactNode } from "react";
import { cn } from "@/lib/utils";

/* ─────────────────────────────────────────────────────────
 * LAYOUT — master/detail split and pane headers.
 *  - MasterDetail: a resizable list pane (default 360, 300…460,
 *    SHELL-2) and a detail pane. The divider is a real
 *    role="separator": drag it, or focus it and use ←/→.
 *  - PaneHeader: the 52px title-bar row at the top of a pane. It is
 *    a Tauri drag region (the overlay title bar has no chrome), so
 *    the window moves by any empty part of it.
 * ───────────────────────────────────────────────────────── */

export function MasterDetail({
  list,
  detail,
  defaultListWidth = 360,
  minListWidth = 300,
  maxListWidth = 460,
  listWidth: controlledWidth,
  onListWidthChange,
  listLabel = "List",
  className,
}: {
  list: ReactNode;
  detail: ReactNode;
  defaultListWidth?: number;
  minListWidth?: number;
  maxListWidth?: number;
  listWidth?: number;
  onListWidthChange?: (width: number) => void;
  /** Names the divider for assistive tech ("Resize the ticket list"). */
  listLabel?: string;
  className?: string;
}) {
  const [inner, setInner] = useState(defaultListWidth);
  const width = controlledWidth ?? inner;
  const drag = useRef<{ x: number; w: number } | null>(null);
  const clamp = (w: number) => Math.round(Math.min(maxListWidth, Math.max(minListWidth, w)));
  const set = (w: number) => {
    const c = clamp(w);
    setInner(c);
    onListWidthChange?.(c);
  };

  const onDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, w: width };
  };
  const onMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    set(drag.current.w + e.clientX - drag.current.x);
  };
  const onUp = () => {
    drag.current = null;
  };
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 40 : 16;
    if (e.key === "ArrowLeft") set(width - step);
    else if (e.key === "ArrowRight") set(width + step);
    else if (e.key === "Home") set(minListWidth);
    else if (e.key === "End") set(maxListWidth);
    else return;
    e.preventDefault();
  };

  return (
    <div className={cn("flex h-full min-h-0 min-w-0 flex-1", className)}>
      <section className="flex min-h-0 shrink-0 flex-col" style={{ width }}>
        {list}
      </section>
      <div
        role="separator"
        aria-orientation="vertical"
        aria-label={listLabel}
        aria-valuenow={width}
        aria-valuemin={minListWidth}
        aria-valuemax={maxListWidth}
        tabIndex={0}
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        onPointerCancel={onUp}
        onKeyDown={onKey}
        className="group/sep relative z-10 -mx-[3px] w-[7px] shrink-0 cursor-col-resize touch-none outline-none"
      >
        <span className="absolute inset-y-0 left-[3px] w-px bg-line transition-colors duration-150 group-hover/sep:bg-line-strong group-focus-visible/sep:w-[2px] group-focus-visible/sep:bg-[var(--focus)] group-active/sep:bg-ink-3" />
      </div>
      <section className="flex min-h-0 min-w-[380px] flex-1 flex-col bg-surface [--titlebar-inset:12px]">{detail}</section>
    </div>
  );
}

export function PaneHeader({
  title,
  subtitle,
  leading,
  actions,
  className,
  children,
}: {
  title?: ReactNode;
  subtitle?: ReactNode;
  leading?: ReactNode;
  actions?: ReactNode;
  className?: string;
  /** Extra rows under the title row (filters, search). */
  children?: ReactNode;
}) {
  return (
    <header className={cn("shrink-0", className)}>
      <div data-tauri-drag-region className="flex h-[52px] items-center gap-2 pr-3" style={{ paddingLeft: "var(--titlebar-inset, 12px)" }}>
        {leading}
        <div data-tauri-drag-region className="min-w-0 flex-1">
          {title ? (
            <h1 data-tauri-drag-region className="truncate text-[14px] font-semibold leading-tight text-ink">
              {title}
            </h1>
          ) : null}
          {subtitle ? (
            <p data-tauri-drag-region className="truncate text-[11.5px] text-ink-2">
              {subtitle}
            </p>
          ) : null}
        </div>
        {actions ? <div className="flex shrink-0 items-center gap-1">{actions}</div> : null}
      </div>
      {children ? <div className="flex flex-col gap-2 px-3 pb-2">{children}</div> : null}
    </header>
  );
}
