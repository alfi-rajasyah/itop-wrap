import type { Pager } from '@/backend'
import { ChevronLeftIcon, ChevronRightIcon, IconButton } from '@/components/kit'

/**
 * The pager Rust describes (LIST-49): nothing, "12 tickets", or ‹ "21–40 of 556" ›. The label
 * and the enabled arrows come from the snapshot.
 */
export function SnapshotPager({ pager, onPage }: { pager: Pager; onPage: (page: number) => void }) {
  if (pager.type === 'none') return null
  if (pager.type === 'count') return <span className="text-[11.5px] tabular-nums text-ink-2">{pager.text}</span>
  return (
    <nav aria-label="Pages" className="flex items-center gap-0.5">
      <IconButton
        label="Previous page"
        icon={<ChevronLeftIcon strokeWidth={2} />}
        size="sm"
        disabled={!pager.canPrevious}
        onClick={() => onPage(pager.page - 1)}
      />
      <span className="min-w-[96px] text-center text-[11.5px] tabular-nums text-ink-2" aria-live="polite">
        {pager.label}
      </span>
      <IconButton
        label="Next page"
        icon={<ChevronRightIcon strokeWidth={2} />}
        size="sm"
        disabled={!pager.canNext}
        onClick={() => onPage(pager.page + 1)}
      />
    </nav>
  )
}
