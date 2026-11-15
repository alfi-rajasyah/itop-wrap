import { useEffect, useState } from "react";

/* Display-only helpers for live counters. The rules for totals, ages and dates live in
 * Rust; these only turn "seconds since a start time" into the shapes the UI ticks
 * (LIST-59): `counter` "HH:MM:SS" and `clock` "h:mm". */

export function formatCounter(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return [h, m, sec].map((n) => String(n).padStart(2, "0")).join(":");
}

export function formatClock(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return `${h}:${String(m).padStart(2, "0")}`;
}

/** Re-renders every `intervalMs` while `active`; returns Date.now(). */
export function useNow(active = true, intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(id);
  }, [active, intervalMs]);
  return now;
}

/** Seconds elapsed since `startedAt` (epoch ms), ticking each second. `undefined` when not running. */
export function useElapsedSeconds(startedAt: number | null | undefined): number | undefined {
  const now = useNow(startedAt != null);
  if (startedAt == null) return undefined;
  return Math.max(0, (now - startedAt) / 1000);
}
