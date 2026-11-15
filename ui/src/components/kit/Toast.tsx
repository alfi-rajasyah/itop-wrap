import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";
import { CheckIcon } from "./icons";

/* ─────────────────────────────────────────────────────────
 * TOAST — short feedback that needs no action ("Copied R-000412",
 * "Saved report.pdf (84 KB)."). A raised surface pill with a check,
 * popping in at the bottom centre and announced politely. For
 * messages the user must read or act on, use NoticeBanner instead.
 * ───────────────────────────────────────────────────────── */

export type ToastItem = { id: number; content: ReactNode; icon?: ReactNode };

export function Toast({ children, icon, className }: { children: ReactNode; icon?: ReactNode; className?: string }) {
  return (
    <div
      className={cn(
        "pointer-events-auto inline-flex h-8 items-center gap-1.5 rounded-full bg-surface pl-2.5 pr-3.5 text-[12.5px] font-medium text-ink shadow-overlay",
        className,
      )}
      style={{ animation: "pop-in 180ms cubic-bezier(0.23,1,0.32,1) both" }}
    >
      <span className="flex shrink-0 text-ink-2 [&_svg]:size-4">{icon ?? <CheckIcon strokeWidth={2.2} />}</span>
      {children}
    </div>
  );
}

/** Fixed region that renders the given toasts. Place once per window. */
export function ToastRegion({ toasts, className }: { toasts: ToastItem[]; className?: string }) {
  if (typeof document === "undefined") return null;
  return createPortal(
    <div
      role="status"
      aria-live="polite"
      className={cn("pointer-events-none fixed inset-x-0 bottom-4 z-[80] flex flex-col items-center gap-2", className)}
    >
      {toasts.map((t) => (
        <Toast key={t.id} icon={t.icon}>
          {t.content}
        </Toast>
      ))}
    </div>,
    document.body,
  );
}

/** Minimal local queue: show(content, ms = 2000). */
export function useToasts() {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const next = useRef(1);
  const timers = useRef(new Set<ReturnType<typeof setTimeout>>());
  useEffect(() => {
    const set = timers.current;
    return () => set.forEach(clearTimeout);
  }, []);
  const dismiss = useCallback((id: number) => setToasts((all) => all.filter((t) => t.id !== id)), []);
  const show = useCallback(
    (content: ReactNode, ms = 2000, icon?: ReactNode) => {
      const id = next.current++;
      setToasts((all) => [...all.slice(-2), { id, content, icon }]);
      const t = setTimeout(() => {
        timers.current.delete(t);
        dismiss(id);
      }, ms);
      timers.current.add(t);
      return id;
    },
    [dismiss],
  );
  return { toasts, show, dismiss };
}
