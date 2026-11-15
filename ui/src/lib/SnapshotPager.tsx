import { ChevronLeftIcon, ChevronRightIcon, IconButton } from '@/components/kit'
import type { Pager } from '@/backend'
import { cn } from '@/lib/utils'

/**
 * The pager under a ticket list or table (LIST-49), drawn from the snapshot's `Pager`:
 * nothing, "12 tickets", or ‹ "21–40 of 556" › with the snapshot's enabled states.
 */
export function SnapshotPager({
  pager,
  onPage,
  className,
}: {
  pager: Pager
  onPage: (page: number) => void
  className?: string
}) {
  if (pager.type === 'none') return null
  if (pager.type === 'count') return <span className={cn('text-[12px] tabular-nums text-ink-2', className)}>{pager.text}</span>
  return (
    <nav aria-label="Pages" className={cn('flex items-center gap-0.5', className)}>
      <IconButton
        label="Previous page"
        icon={<ChevronLeftIcon strokeWidth={2} />}
        size="sm"
        disabled={!pager.canPrevious}
        onClick={() => onPage(pager.page - 1)}
      />
      <span className="min-w-[88px] text-center text-[12px] tabular-nums text-ink-2" aria-live="polite">
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
