import { useEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { CheckIcon } from "./icons";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * REF + DURATION — the identity motif. Ticket refs and durations
 * are JetBrains Mono with tabular figures, one weight, everywhere.
 * A running duration is bold (never coloured).
 * ───────────────────────────────────────────────────────── */

export function Ref({ children, className }: { children: ReactNode; className?: string }) {
  return <span className={cn("mono-figures whitespace-nowrap text-[11.5px] text-ink-2", className)}>{children}</span>;
}

export function Duration({
  children,
  running = false,
  className,
}: {
  children: ReactNode;
  running?: boolean;
  className?: string;
}) {
  return (
    <span className={cn("mono-figures whitespace-nowrap text-[11.5px]", running ? "font-bold text-ink" : "text-ink-2", className)}>
      {children}
    </span>
  );
}

/** "✓ Copied R-000412": the 2-second confirmation chip (DET-10). Announced politely. */
export function CopiedChip({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span
      role="status"
      className={cn(
        "inline-flex h-6 items-center gap-1 rounded-full bg-surface pl-1.5 pr-2.5 text-[12px] font-medium text-ink shadow-raised",
        className,
      )}
      style={{ animation: "pop-in 160ms cubic-bezier(0.23,1,0.32,1) both" }}
    >
      <CheckIcon className="size-3.5" strokeWidth={2.2} />
      {children}
    </span>
  );
}

async function defaultCopy(text: string) {
  await navigator.clipboard?.writeText(text);
}

/**
 * The clickable ref: copies it, then reads "Copied" for 2 s (like Code Block's copy
 * button). Tooltip "Copy R-000412"; accessible name "R-000412, copy ref".
 */
export function RefChip({
  value,
  onCopy = defaultCopy,
  copiedMs = 2000,
  size = "md",
  className,
}: {
  value: string;
  /** Puts the text on the clipboard. Defaults to navigator.clipboard. */
  onCopy?: (value: string) => void | Promise<void>;
  copiedMs?: number;
  size?: "sm" | "md";
  className?: string;
}) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);

  const copy = async () => {
    try {
      await onCopy(value);
      setCopied(true);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), copiedMs);
    } catch {
      setCopied(false);
    }
  };

  return (
    <Tooltip content={copied ? "Copied" : `Copy ${value}`} describe={false}>
      <button
        type="button"
        onClick={copy}
        aria-label={`${value}, copy ref`}
        className={cn(
          "mono-figures relative -mx-1 inline-flex items-center rounded-[5px] px-1 text-ink-2 transition-colors duration-100 hover:bg-hover-2 hover:text-ink",
          size === "sm" ? "h-5 text-[11.5px]" : "h-6 text-[12px]",
          className,
        )}
      >
        <span className={cn("transition-opacity duration-150", copied && "opacity-0")}>{value}</span>
        <span
          aria-hidden={!copied}
          className={cn(
            "absolute inset-0 flex items-center justify-center gap-0.5 font-sans font-medium text-ink transition-opacity duration-150",
            copied ? "opacity-100" : "opacity-0",
          )}
        >
          <CheckIcon className="size-3" strokeWidth={2.4} />
          Copied
        </span>
        <span className="sr-only" role="status">
          {copied ? `Copied ${value}` : ""}
        </span>
      </button>
    </Tooltip>
  );
}
