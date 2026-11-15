import type { SVGProps } from "react";

/* Icons: iconoir-react everywhere (free, MIT). The paid Central Icons used by
 * Sidebar Nav are replaced with iconoir equivalents. Two glyphs iconoir lacks are
 * drawn here in its style (24 grid, 1.5 stroke, round caps). */

export {
  Archive as TrayIcon,
  Attachment as PaperclipIcon,
  Building as BuildingIcon,
  ChatBubbleQuestion as UserRequestIcon,
  Check as CheckIcon,
  Copy as CopyIcon,
  FilterList as ViewIcon,
  Globe as GlobeIcon,
  Group as TeamIcon,
  InfoCircle as InfoIcon,
  Lock as LockIcon,
  MediaImage as ImageIcon,
  MoreHoriz as MoreIcon,
  NavArrowDown as ChevronDownIcon,
  NavArrowLeft as ChevronLeftIcon,
  NavArrowRight as ChevronRightIcon,
  OpenInWindow as OpenWindowIcon,
  OpenNewWindow as ExternalIcon,
  Refresh as RefreshIcon,
  Search as SearchIcon,
  SendDiagonal as SendIcon,
  Server as ServerIcon,
  Timer as StopwatchIcon,
  User as PersonIcon,
  UserSquare as PeopleIcon,
  WarningTriangle as IncidentIcon,
  WarningCircle as ErrorIcon,
  Xmark as XIcon,
  EditPencil as NewTicketIcon,
  LogOut as SignOutIcon,
  Calendar as CalendarIcon,
  SidebarCollapse as SidebarIcon,
} from "iconoir-react";

/** Filled stop square: the running timer's button (stopwatch ↔ stop). */
export function StopIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg width="1.5em" height="1.5em" viewBox="0 0 24 24" fill="none" aria-hidden {...props}>
      <rect x="6.5" y="6.5" width="11" height="11" rx="2" fill="currentColor" />
    </svg>
  );
}

/** Up/down chevron pair for dropdown fields (macOS pop-up style). */
export function ChevronUpDownIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg width="1.5em" height="1.5em" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden {...props}>
      <path d="M8 9.5 12 5.5l4 4M8 14.5l4 4 4-4" />
    </svg>
  );
}
