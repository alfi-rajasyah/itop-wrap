import { cn } from "@/lib/utils";

/* Small activity ring, adapted from Task Rows' SpinnerRing (track in --line, a 28% arc
 * in the current colour, spin 1.1s). Under reduced motion it holds still. */
export function Spinner({ size = 14, className, label }: { size?: number; className?: string; label?: string }) {
  const stroke = size <= 14 ? 1.75 : 2;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  return (
    <span
      role={label ? "status" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      className={cn("relative inline-flex shrink-0", className)}
      style={{ width: size, height: size }}
    >
      <svg width={size} height={size} className="absolute inset-0" style={{ animation: "spin 1.1s linear infinite" }}>
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="currentColor" strokeOpacity={0.22} strokeWidth={stroke} />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke="currentColor"
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={`${c * 0.28} ${c * 0.72}`}
        />
      </svg>
    </span>
  );
}
