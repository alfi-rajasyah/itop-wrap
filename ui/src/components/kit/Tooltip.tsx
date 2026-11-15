import {
  cloneElement,
  isValidElement,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactElement,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";

/* ─────────────────────────────────────────────────────────
 * TOOLTIP — Beautiful UI's tooltip surface (--tooltip-bg/-fg,
 * overlay shadow, 8px radius, 11.5px text), shown after a short
 * hover delay or at once on keyboard focus. Moving between triggers
 * shows the next one instantly. Esc hides it.
 *
 * The wrapper span receives the pointer events, so a tooltip still
 * explains a disabled button (Beautiful UI's Button sets
 * pointer-events:none when disabled).
 * ───────────────────────────────────────────────────────── */

const OPEN_DELAY = 550;
const WARM_WINDOW = 400;
let lastHiddenAt = 0;

export type TooltipSide = "top" | "bottom";

export function Tooltip({
  content,
  children,
  side = "bottom",
  className,
  describe = true,
}: {
  /** Tooltip text. Nothing renders when empty. */
  content: ReactNode;
  children: ReactNode;
  side?: TooltipSide;
  className?: string;
  /** Add aria-describedby to the child. Turn off when the tooltip only repeats the
   *  child's accessible name (icon buttons), so screen readers don't say it twice. */
  describe?: boolean;
}) {
  const id = useId();
  const wrapRef = useRef<HTMLSpanElement>(null);
  const tipRef = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number; side: TooltipSide } | null>(null);

  const show = (immediate = false) => {
    if (!content) return;
    clearTimeout(timer.current);
    const warm = Date.now() - lastHiddenAt < WARM_WINDOW;
    if (immediate || warm) setOpen(true);
    else timer.current = setTimeout(() => setOpen(true), OPEN_DELAY);
  };
  const hide = () => {
    clearTimeout(timer.current);
    setOpen((was) => {
      if (was) lastHiddenAt = Date.now();
      return false;
    });
  };

  useEffect(() => () => clearTimeout(timer.current), []);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") hide();
    };
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  }, [open]);

  useLayoutEffect(() => {
    if (!open) {
      setPos(null);
      return;
    }
    const wrap = wrapRef.current;
    const tip = tipRef.current;
    if (!wrap || !tip) return;
    const r = wrap.getBoundingClientRect();
    const tw = tip.offsetWidth;
    const th = tip.offsetHeight;
    let s: TooltipSide = side;
    if (s === "bottom" && r.bottom + 6 + th > window.innerHeight - 4) s = "top";
    if (s === "top" && r.top - 6 - th < 4) s = "bottom";
    const top = s === "bottom" ? r.bottom + 6 : r.top - 6 - th;
    const left = Math.max(6, Math.min(r.left + r.width / 2 - tw / 2, window.innerWidth - tw - 6));
    setPos({ top, left, side: s });
  }, [open, side, content]);

  const child =
    describe && content && isValidElement(children)
      ? cloneElement(children as ReactElement<{ "aria-describedby"?: string }>, {
          "aria-describedby": open ? id : undefined,
        })
      : children;

  return (
    <span
      ref={wrapRef}
      className={cn("inline-flex", className)}
      onPointerEnter={(e) => {
        if (e.pointerType === "mouse") show();
      }}
      onPointerLeave={hide}
      onPointerDown={hide}
      onFocus={(e) => {
        // only keyboard focus shows it right away; a click's focus shouldn't
        if ((e.target as HTMLElement).matches?.(":focus-visible")) show(true);
      }}
      onBlur={hide}
    >
      {child}
      {open && content && typeof document !== "undefined"
        ? createPortal(
            <div
              ref={tipRef}
              id={id}
              role="tooltip"
              className="pointer-events-none fixed z-[70] max-w-[280px] rounded-[8px] px-2 py-1 text-[11.5px] leading-[1.35] shadow-overlay"
              style={{
                top: pos?.top ?? -9999,
                left: pos?.left ?? -9999,
                visibility: pos ? "visible" : "hidden",
                color: "var(--tooltip-fg)",
                background: "var(--tooltip-bg)",
                border: "1px solid var(--tooltip-border)",
                animation: "fade-in 120ms ease-out both",
              }}
            >
              {content}
            </div>,
            document.body,
          )
        : null}
    </span>
  );
}
