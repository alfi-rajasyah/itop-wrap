import {
  forwardRef,
  useId,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
  type TextareaHTMLAttributes,
} from "react";
import { cn } from "@/lib/utils";
import { CheckIcon, InfoIcon } from "./icons";
import { Tooltip } from "./Tooltip";

/* ─────────────────────────────────────────────────────────
 * FORM — field wrapper, text input, textarea, checkbox, segmented
 * control and the ⓘ hint. Controls use Beautiful UI's inset field
 * (bg-field, hairline ring, control radius) from Chat's composer and
 * Fine-tune Card; the segmented control is Fine-tune Card's sliding
 * thumb on a grey track.
 *
 * Required fields say "Required" next to the label from the start;
 * it turns red only after a confirm attempt (LC-8). Errors sit in
 * reserved space so showing one never moves the buttons.
 * ───────────────────────────────────────────────────────── */

export const fieldControlClass = cn(
  "w-full rounded-control bg-field px-2.5 text-[13px] text-ink shadow-hairline outline-none",
  "placeholder:text-ink-2/80 transition-[box-shadow,background-color] duration-150",
  "hover:bg-hover-2 focus:bg-field focus:shadow-[0_0_0_1.5px_var(--ink-2)]",
  "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--focus)]",
  "disabled:cursor-default disabled:opacity-55 read-only:hover:bg-field",
  "aria-invalid:shadow-[0_0_0_1.5px_var(--danger)]",
);

export type FieldIds = { id: string; describedBy?: string; invalid: boolean; required: boolean };

export function Field({
  label,
  id: idProp,
  required = false,
  requiredError = false,
  hint,
  error,
  reserveErrorSpace = true,
  className,
  children,
}: {
  label: ReactNode;
  id?: string;
  required?: boolean;
  /** Turn "Required" red (after a confirm attempt with the field empty). */
  requiredError?: boolean;
  /** Help text behind an ⓘ button (DESIGN.md "Hints behind ⓘ"). */
  hint?: string;
  error?: ReactNode;
  reserveErrorSpace?: boolean;
  className?: string;
  /** Either nodes (wire `id` yourself) or a render function given the ids to use. */
  children: ReactNode | ((ids: FieldIds) => ReactNode);
}) {
  const auto = useId();
  const id = idProp ?? `f-${auto}`;
  const errorId = `${id}-error`;
  const invalid = !!error || requiredError;
  const ids: FieldIds = { id, describedBy: error ? errorId : undefined, invalid, required };
  return (
    <div className={cn("flex min-w-0 flex-col gap-1.5", className)}>
      <div className="flex items-center gap-1.5">
        <label htmlFor={id} id={`${id}-label`} className="text-[12.5px] font-medium text-ink">
          {label}
        </label>
        {required ? (
          <span className={cn("text-[11.5px]", requiredError ? "font-medium text-danger" : "text-ink-2")}>Required</span>
        ) : null}
        {hint ? <InfoHint text={hint} /> : null}
      </div>
      {typeof children === "function" ? children(ids) : children}
      {reserveErrorSpace || error ? (
        <p id={errorId} className="min-h-[16px] text-[12px] leading-[16px] text-danger" aria-live="polite">
          {error}
        </p>
      ) : null}
    </div>
  );
}

/** ⓘ button whose tooltip and accessible name hold the help text. */
export function InfoHint({ text }: { text: string }) {
  return (
    <Tooltip content={text} describe={false}>
      <button
        type="button"
        aria-label={text}
        className="inline-flex size-4 items-center justify-center rounded-full text-ink-2 hover:text-ink"
      >
        <InfoIcon className="size-3.5" />
      </button>
    </Tooltip>
  );
}

export const TextInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement> & { invalid?: boolean }>(
  function TextInput({ className, invalid, ...props }, ref) {
    return <input ref={ref} aria-invalid={invalid || undefined} className={cn(fieldControlClass, "h-8", className)} {...props} />;
  },
);

export const TextArea = forwardRef<
  HTMLTextAreaElement,
  TextareaHTMLAttributes<HTMLTextAreaElement> & { invalid?: boolean; minHeight?: number }
>(function TextArea({ className, invalid, minHeight = 90, style, ...props }, ref) {
  return (
    <textarea
      ref={ref}
      aria-invalid={invalid || undefined}
      className={cn(fieldControlClass, "resize-y py-2 leading-[1.45]", className)}
      style={{ minHeight, ...style }}
      {...props}
    />
  );
});

export function Checkbox({
  checked,
  onChange,
  label,
  disabled,
  id,
  className,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: ReactNode;
  disabled?: boolean;
  id?: string;
  className?: string;
}) {
  const auto = useId();
  const cid = id ?? `cb-${auto}`;
  return (
    <label htmlFor={cid} className={cn("inline-flex select-none items-center gap-2 text-[13px] text-ink", disabled && "opacity-55", className)}>
      <span className="relative inline-flex">
        <input
          id={cid}
          type="checkbox"
          checked={checked}
          disabled={disabled}
          onChange={(e) => onChange(e.target.checked)}
          className="peer absolute inset-0 size-4 cursor-pointer opacity-0 disabled:cursor-default"
        />
        <span
          aria-hidden
          className={cn(
            "flex size-4 items-center justify-center rounded-[5px] border transition-colors duration-100",
            "peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-[var(--focus)]",
            checked ? "border-ink bg-ink text-surface" : "border-line-strong bg-surface text-transparent",
          )}
        >
          <CheckIcon className="size-3" strokeWidth={2.6} />
        </span>
      </span>
      {label}
    </label>
  );
}

export type SegmentOption<T extends string> = { value: T; label: ReactNode; "aria-label"?: string };

/** Segmented control (radiogroup): "User request" / "Incident". ←/→ move the choice. */
export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  label,
  size = "md",
  className,
}: {
  options: SegmentOption<T>[];
  value: T;
  onChange: (value: T) => void;
  /** Accessible name of the group. */
  label: string;
  size?: "sm" | "md";
  className?: string;
}) {
  const index = Math.max(0, options.findIndex((o) => o.value === value));
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const delta = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!delta) return;
    e.preventDefault();
    const next = options[(index + delta + options.length) % options.length]!;
    onChange(next.value);
    const el = e.currentTarget.querySelector<HTMLButtonElement>(`[data-seg="${next.value}"]`);
    el?.focus();
  };
  return (
    <div
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKey}
      className={cn("relative grid rounded-control bg-field p-0.5 shadow-hairline", className)}
      style={{ gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` }}
    >
      <span
        aria-hidden
        className="absolute inset-y-0.5 rounded-[6px] bg-surface shadow-btn transition-transform duration-200 dark:bg-hover-2"
        style={{
          width: `calc((100% - 4px) / ${options.length})`,
          left: 2,
          transform: `translateX(${index * 100}%)`,
          transitionTimingFunction: "cubic-bezier(0.23, 1, 0.32, 1)",
        }}
      />
      {options.map((o, i) => (
        <button
          key={o.value}
          data-seg={o.value}
          type="button"
          role="radio"
          aria-checked={i === index}
          aria-label={o["aria-label"]}
          tabIndex={i === index ? 0 : -1}
          onClick={() => onChange(o.value)}
          className={cn(
            "relative z-10 flex items-center justify-center gap-1.5 whitespace-nowrap rounded-[6px] px-2.5 font-medium transition-colors duration-150",
            size === "sm" ? "h-6 text-[12px]" : "h-7 text-[12.5px]",
            i === index ? "text-ink" : "text-ink-2 hover:text-ink",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
