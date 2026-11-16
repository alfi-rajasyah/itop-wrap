// NOT from the site: Button.tsx imports `cn` from "@/lib/utils", which the
// registry does not ship. This is the standard shadcn/ui helper it expects.
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
