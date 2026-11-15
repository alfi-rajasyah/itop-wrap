import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Spinner } from "./Spinner";
import { Tooltip, type TooltipSide } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * ICON BUTTON — Beautiful UI's `.primitive-icon-button` (28px,
 * control radius, ink-2 → ink on hover) as used in Chat's header and
 * Sidebar Nav's controls. Always has an accessible name and a tooltip
 * (R-39). `variant="field"` is the header-control look: a filled
 * neutral chip beside the primary action (timer, More).
 * ───────────────────────────────────────────────────────── */

export type IconButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "aria-label"> & {
  /** Accessible name. Also the tooltip unless `tooltip` is given. */
  label: string;
  icon: ReactNode;
  tooltip?: ReactNode;
  tooltipSide?: TooltipSide;
  size?: "xs" | "sm" | "md";
  variant?: "quiet" | "field";
  busy?: boolean;
  /** Optional text after the icon (a live counter, a count). */
  children?: ReactNode;
};

const SIZES = {
  xs: "h-5.5 min-w-5.5 rounded-[6px] [&_svg]:size-3.5",
  sm: "h-6.5 min-w-6.5 rounded-[7px] [&_svg]:size-4",
  md: "h-7 min-w-7 rounded-control [&_svg]:size-[17px]",
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { label, icon, tooltip, tooltipSide, size = "md", variant = "quiet", busy, disabled, className, children, type = "button", ...props },
  ref,
) {
  return (
    <Tooltip content={tooltip ?? label} side={tooltipSide} describe={tooltip != null && tooltip !== label}>
      <button
        ref={ref}
        type={type}
        aria-label={label}
        disabled={disabled || busy}
        className={cn(
          "inline-flex shrink-0 items-center justify-center gap-1.5 transition-[background-color,color,transform] duration-150",
          "active:scale-[0.94] disabled:pointer-events-none disabled:opacity-40",
          SIZES[size],
          children != null && "px-2",
          variant === "quiet"
            ? "text-ink-2 hover:bg-hover-2 hover:text-ink aria-expanded:bg-hover-2 aria-expanded:text-ink aria-pressed:bg-hover-2 aria-pressed:text-ink"
            : "bg-hover-2 text-ink hover:bg-line-strong aria-expanded:bg-line-strong",
          className,
        )}
        {...props}
      >
        {busy ? <Spinner size={size === "xs" ? 12 : 14} /> : icon}
        {children}
      </button>
    </Tooltip>
  );
});
