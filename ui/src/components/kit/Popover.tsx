import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type HTMLAttributes,
  type ReactNode,
  type RefObject,
} from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";

/* ─────────────────────────────────────────────────────────
 * POPOVER — the floating surface shared by menus, dropdowns and
 * tooltips. Beautiful UI's menu card (12px radius, overlay shadow,
 * 140ms pop-in) portalled to <body> and anchored to a trigger, so it
 * escapes scroll containers and sheets. Opens below; flips above when
 * it doesn't fit (KEY-11).
 * ───────────────────────────────────────────────────────── */

export type Placement = "bottom-start" | "bottom-end" | "top-start" | "top-end";

type Position = { top: number; left: number; maxHeight: number; flipped: boolean; width?: number };

const GAP = 6;
const MARGIN = 8;

export function useAnchoredPosition(
  anchorRef: RefObject<HTMLElement | null>,
  floatingRef: RefObject<HTMLElement | null>,
  open: boolean,
  { placement = "bottom-start", matchWidth = false, maxHeight = 320 }: { placement?: Placement; matchWidth?: boolean; maxHeight?: number },
) {
  const [pos, setPos] = useState<Position | null>(null);

  const update = useCallback(() => {
    const anchor = anchorRef.current;
    if (!anchor) return;
    const r = anchor.getBoundingClientRect();
    const fl = floatingRef.current;
    const fh = fl ? fl.scrollHeight : 0;
    const fw = matchWidth ? r.width : fl ? fl.offsetWidth : 0;
    const vh = window.innerHeight;
    const vw = window.innerWidth;
    const below = vh - r.bottom - GAP - MARGIN;
    const above = r.top - GAP - MARGIN;
    const wantTop = placement.startsWith("top");
    const needed = Math.min(fh || maxHeight, maxHeight);
    const flipped = wantTop ? above < needed && below > above : below < needed && above > below;
    const useTop = wantTop !== flipped;
    const room = Math.max(80, Math.min(maxHeight, useTop ? above : below));
    const h = Math.min(fh || room, room);
    const top = useTop ? r.top - GAP - h : r.bottom + GAP;
    let left = placement.endsWith("end") ? r.right - fw : r.left;
    left = Math.max(MARGIN, Math.min(left, vw - fw - MARGIN));
    setPos({ top, left, maxHeight: room, flipped: useTop, width: matchWidth ? r.width : undefined });
  }, [anchorRef, floatingRef, placement, matchWidth, maxHeight]);

  useLayoutEffect(() => {
    if (!open) {
      setPos(null);
      return;
    }
    update();
    // measure once more after the content paints (fonts, async rows)
    const raf = requestAnimationFrame(update);
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [open, update]);

  return { pos, update };
}

/** Closes on a pointerdown outside the floating element and the anchor. */
export function useDismiss(
  open: boolean,
  refs: RefObject<HTMLElement | null>[],
  onDismiss: () => void,
) {
  const cb = useRef(onDismiss);
  cb.current = onDismiss;
  useEffect(() => {
    if (!open) return;
    const onDown = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (refs.some((r) => r.current?.contains(target))) return;
      cb.current();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => document.removeEventListener("pointerdown", onDown, true);
    // refs are stable ref objects, so only `open` matters
  }, [open]);
}

export function Popover({
  anchorRef,
  open,
  onDismiss,
  placement = "bottom-start",
  matchWidth = false,
  maxHeight = 320,
  className,
  style,
  children,
  floatingRef: externalRef,
  ...rest
}: {
  anchorRef: RefObject<HTMLElement | null>;
  open: boolean;
  onDismiss?: () => void;
  placement?: Placement;
  matchWidth?: boolean;
  maxHeight?: number;
  className?: string;
  style?: CSSProperties;
  children: ReactNode;
  floatingRef?: RefObject<HTMLDivElement | null>;
} & Omit<HTMLAttributes<HTMLDivElement>, "style" | "className" | "children">) {
  const innerRef = useRef<HTMLDivElement>(null);
  const ref = externalRef ?? innerRef;
  const { pos } = useAnchoredPosition(anchorRef, ref, open, { placement, matchWidth, maxHeight });
  useDismiss(open, [ref, anchorRef as RefObject<HTMLElement | null>], () => onDismiss?.());

  if (!open || typeof document === "undefined") return null;
  return createPortal(
    <div
      ref={ref}
      className={cn(
        "kit-scroll fixed z-[60] overflow-y-auto rounded-[12px] bg-surface p-1.5 text-ink shadow-overlay",
        className,
      )}
      style={{
        top: pos?.top ?? -9999,
        left: pos?.left ?? -9999,
        maxHeight: pos?.maxHeight ?? maxHeight,
        width: pos?.width,
        visibility: pos ? "visible" : "hidden",
        animation: "pop-in 140ms cubic-bezier(0.23,1,0.32,1) both",
        transformOrigin: pos?.flipped ? "bottom left" : "top left",
        ...style,
      }}
      {...rest}
    >
      {children}
    </div>,
    document.body,
  );
}
