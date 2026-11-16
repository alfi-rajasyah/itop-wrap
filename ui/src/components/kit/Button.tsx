import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { buttonVariants } from "@/components/atoms/Button";
import { cn } from "@/lib/utils";
import { Spinner } from "./Spinner";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * BUTTON — Beautiful UI's pill Button atom (atoms/Button), plus:
 *  - variant "need": the ONE teal filled action, only for the
 *    primary action on a ticket that needs you (DESIGN.md).
 *  - size "block": the full-width 36px filled button (sign in,
 *    sheet "Assign to me").
 *  - busy: a mini spinner in place of the icon; the button is
 *    disabled and keeps its width.
 *  - tooltip: shown even while disabled.
 *  - variant "danger": red filled, only for an action that can't be
 *    undone from the app (Resolve and Close).
 * "accent" and "success" from the atom are not exposed: blue and
 * green have no meaning here.
 * ───────────────────────────────────────────────────────── */

export type KitButtonVariant = "primary" | "secondary" | "ghost" | "quiet" | "need" | "danger";
export type KitButtonSize = "xs" | "sm" | "md" | "block";

export type ButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> & {
  variant?: KitButtonVariant;
  size?: KitButtonSize;
  /** Leading icon (16px). Replaced by a spinner while busy. */
  icon?: ReactNode;
  busy?: boolean;
  tooltip?: ReactNode;
  children?: ReactNode;
};

const EXTRA: Record<string, string> = {
  need: "bg-need text-on-need hover:opacity-90 dark:bg-need dark:text-on-need shadow-[inset_0_1px_0_rgba(255,255,255,0.14)]",
  danger: "bg-danger text-on-danger hover:opacity-90 dark:bg-danger dark:text-on-danger shadow-[inset_0_1px_0_rgba(255,255,255,0.14)]",
};

export function kitButtonClass(variant: KitButtonVariant = "secondary", size: KitButtonSize = "sm", className?: string) {
  const base = variant === "need" || variant === "danger" ? "primary" : variant;
  const atomSize = size === "block" ? "md" : size;
  return cn(
    buttonVariants({ variant: base, size: atomSize }),
    EXTRA[variant],
    size === "block" && "h-9 w-full rounded-[10px] text-[13px] font-semibold",
    className,
  );
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "sm", icon, busy = false, tooltip, className, disabled, children, type = "button", ...props },
  ref,
) {
  const button = (
    <button
      ref={ref}
      type={type}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      className={kitButtonClass(variant, size, className)}
      {...props}
    >
      {busy ? <Spinner size={13} /> : icon ? <span className="-ml-0.5 flex shrink-0 [&>svg]:size-[15px]">{icon}</span> : null}
      {children}
    </button>
  );
  return tooltip ? (
    <Tooltip content={tooltip} className={size === "block" ? "flex w-full" : undefined}>
      {button}
    </Tooltip>
  ) : (
    button
  );
});
