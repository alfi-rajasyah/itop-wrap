import { BuildingIcon, IconButton, XIcon } from '@/components/kit'

/**
 * "Config items in <org>" / "People in <org>" above a list (CMDB-7, PPL-5), with the × that
 * goes back to searching.
 */
export function ScopeBar({ text, onClear }: { text: string; onClear: () => void }) {
  return (
    <div className="mx-3 mb-2 flex h-8 items-center gap-2 rounded-control bg-inset pl-2.5 pr-1 text-[12.5px] text-ink shadow-hairline">
      <BuildingIcon aria-hidden className="size-4 shrink-0 text-ink-2" />
      <span className="min-w-0 flex-1 truncate">{text}</span>
      <IconButton label="Clear organization" tooltip="Show search instead" icon={<XIcon strokeWidth={2} />} size="xs" onClick={onClear} />
    </div>
  )
}
