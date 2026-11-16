import { useEffect, useState } from "react";

export type ThemeMode = "system" | "light" | "dark";

const QUERY = "(prefers-color-scheme: dark)";

function systemPrefersDark(): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia(QUERY).matches
    : false;
}

/** Puts `.dark` on <html> (Beautiful UI's tokens switch on it). Swaps every token in
 *  one repaint: `.theme-switching` freezes transitions for a frame so colours don't fade. */
function applyDark(dark: boolean) {
  const root = document.documentElement;
  if (root.classList.contains("dark") === dark) return;
  root.classList.add("theme-switching");
  root.classList.toggle("dark", dark);
  root.style.colorScheme = dark ? "dark" : "light";
  requestAnimationFrame(() => requestAnimationFrame(() => root.classList.remove("theme-switching")));
}

/**
 * Keeps the `.dark` class on <html> in sync with the system appearance
 * (`prefers-color-scheme`), or forces a mode. Returns whether dark is in effect.
 * Call it once near the root of each window.
 */
export function useSystemTheme(mode: ThemeMode = "system"): boolean {
  const [systemDark, setSystemDark] = useState(systemPrefersDark);

  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const mql = window.matchMedia(QUERY);
    const onChange = () => setSystemDark(mql.matches);
    onChange();
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  const dark = mode === "system" ? systemDark : mode === "dark";

  useEffect(() => {
    applyDark(dark);
  }, [dark]);

  return dark;
}
