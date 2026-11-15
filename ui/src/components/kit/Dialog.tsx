import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";
import { Button, type KitButtonVariant } from "./Button";

/* ─────────────────────────────────────────────────────────
 * DIALOG / SHEET — a modal on Beautiful UI's window surface (14px
 * radius, overlay shadow, pop-in), the same card grammar as Approval
 * Card: title, quiet subtitle, one column of fields, buttons bottom
 * right named for their action (LC-7).
 *
 *  - Esc cancels (an open dropdown or menu swallows Esc first).
 *  - ⌘↩ (Ctrl↩) confirms unless the confirm button is disabled/busy.
 *  - Focus moves in on open, Tab is trapped, focus returns on close.
 *  - Cancel discards without asking: there is no backdrop-click close,
 *    so a stray click can't lose typed text.
 * ───────────────────────────────────────────────────────── */

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"]), [contenteditable="true"]';

export type DialogProps = {
  open: boolean;
  title: ReactNode;
  /** Quiet line under the title, e.g. the ticket title (2 lines max). */
  subtitle?: ReactNode;
  children?: ReactNode;
  onCancel: () => void;
  onConfirm?: () => void;
  confirmLabel?: ReactNode;
  cancelLabel?: ReactNode;
  confirmVariant?: KitButtonVariant;
  confirmDisabled?: boolean;
  confirmBusy?: boolean;
  confirmTooltip?: ReactNode;
  /** Error under the form, above the buttons (space reserved so buttons don't jump). */
  error?: ReactNode;
  /** Left side of the footer ("Clear form"). */
  footerStart?: ReactNode;
  /** Replace the whole button row. */
  footer?: ReactNode;
  width?: number;
  role?: "dialog" | "alertdialog";
  initialFocusRef?: RefObject<HTMLElement | null>;
  className?: string;
};

export function Dialog({
  open,
  title,
  subtitle,
  children,
  onCancel,
  onConfirm,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  confirmVariant = "primary",
  confirmDisabled = false,
  confirmBusy = false,
  confirmTooltip,
  error,
  footerStart,
  footer,
  width = 420,
  role = "dialog",
  initialFocusRef,
  className,
}: DialogProps) {
  const titleId = useId();
  const descId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const restoreRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) return;
    restoreRef.current = document.activeElement as HTMLElement | null;
    const raf = requestAnimationFrame(() => {
      const panel = panelRef.current;
      if (!panel) return;
      const target =
        initialFocusRef?.current ??
        panel.querySelector<HTMLElement>("[data-autofocus]") ??
        panel.querySelector<HTMLElement>(`[data-dialog-body] :is(${FOCUSABLE})`) ??
        panel;
      target.focus();
    });
    return () => {
      cancelAnimationFrame(raf);
      restoreRef.current?.focus?.();
    };
  }, [open, initialFocusRef]);

  if (!open || typeof document === "undefined") return null;

  const canConfirm = !!onConfirm && !confirmDisabled && !confirmBusy;

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape" && !e.defaultPrevented) {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
      return;
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      if (canConfirm) onConfirm!();
      return;
    }
    if (e.key === "Tab") {
      const panel = panelRef.current;
      if (!panel) return;
      const list = Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => !el.closest("[hidden],[inert]"));
      if (!list.length) {
        e.preventDefault();
        return;
      }
      const first = list[0]!;
      const last = list[list.length - 1]!;
      if (e.shiftKey && (document.activeElement === first || document.activeElement === panel)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  };

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto px-4 pb-8 pt-[10vh]">
      <div
        aria-hidden
        className="fixed inset-0 bg-[oklch(0.2_0.01_264/0.28)] dark:bg-[oklch(0_0_0/0.5)]"
        style={{ animation: "fade-in 160ms ease-out both" }}
      />
      <div
        ref={panelRef}
        role={role}
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={subtitle ? descId : undefined}
        tabIndex={-1}
        onKeyDown={onKeyDown}
        className={cn("relative flex max-w-full flex-col rounded-[14px] bg-surface text-ink shadow-overlay outline-none", className)}
        style={{ width, animation: "pop-in 180ms cubic-bezier(0.23,1,0.32,1) both", transformOrigin: "top center" }}
      >
        <div className="px-5 pb-1 pt-4">
          <h2 id={titleId} className="text-[15px] font-semibold leading-snug text-ink">
            {title}
          </h2>
          {subtitle ? (
            <p id={descId} className="mt-0.5 line-clamp-2 text-[13px] leading-snug text-ink-2">
              {subtitle}
            </p>
          ) : null}
        </div>
        {children ? (
          <div data-dialog-body className="flex flex-col gap-1.5 px-5 pb-1 pt-3">
            {children}
          </div>
        ) : null}
        {children || error ? (
          <div className="min-h-[18px] px-5" aria-live="polite">
            {error ? <p className="text-[12px] leading-snug text-danger">{error}</p> : null}
          </div>
        ) : (
          <div className="h-3" />
        )}
        <div className="flex items-center gap-2 px-5 pb-4 pt-2">
          {footer ?? (
            <>
              <div className="mr-auto flex items-center">{footerStart}</div>
              <Button variant="secondary" size="md" onClick={onCancel}>
                {cancelLabel}
              </Button>
              {onConfirm ? (
                <Button
                  variant={confirmVariant}
                  size="md"
                  onClick={onConfirm}
                  disabled={confirmDisabled}
                  busy={confirmBusy}
                  tooltip={confirmTooltip}
                  aria-keyshortcuts="Meta+Enter"
                >
                  {confirmLabel}
                </Button>
              ) : null}
            </>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}
