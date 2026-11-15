import type { ReactNode } from 'react'
import { Button, LoadingState, Spinner } from '@/components/kit'
import { cn } from '@/lib/utils'

/* Small, calm states shared by the detail panes and list columns. All text comes from the
 * snapshot (or is fixed copy like "Retry"). */

/** Spinner + "Loading the ticket from iTop" under a header: inline, left-aligned. */
export function InlineLoading({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div role="status" className={cn('flex items-center gap-2 py-2 text-[13px] text-ink-2', className)}>
      <Spinner size={14} />
      <span>{children}</span>
    </div>
  )
}

/** A failed load in a detail: the message in the error colour, then Retry. */
export function InlineError({
  children,
  onRetry,
  className,
}: {
  children: ReactNode
  onRetry?: () => void
  className?: string
}) {
  return (
    <div role="alert" className={cn('flex flex-col items-start gap-2 py-1', className)}>
      <p className="max-w-[68ch] text-[13px] leading-[1.5] text-danger">{children}</p>
      {onRetry ? (
        <Button size="sm" variant="secondary" onClick={onRetry}>
          Retry
        </Button>
      ) : null}
    </div>
  )
}

/** A list column's message (idle hint, nothing found, failure), centred in the column. */
export function PaneMessage({
  children,
  tone = 'quiet',
  action,
  className,
}: {
  children: ReactNode
  tone?: 'quiet' | 'error'
  action?: ReactNode
  className?: string
}) {
  return (
    <div
      role={tone === 'error' ? 'alert' : undefined}
      className={cn('flex flex-1 flex-col items-center justify-center gap-3 px-6 py-10 text-center', className)}
    >
      <p className={cn('max-w-[36ch] text-[13px] leading-[1.5]', tone === 'error' ? 'text-danger' : 'text-ink-2')}>{children}</p>
      {action}
    </div>
  )
}

/** A list column that is loading ("Searching iTop"). */
export function PaneLoading({ label }: { label: string }) {
  return <LoadingState label={label} className="flex-1" />
}

/** The detail column with nothing selected (SHELL-7). */
export function DetailPlaceholder({ text }: { text: string }) {
  return (
    <div className="flex flex-1 items-center justify-center px-8 text-center">
      <p className="max-w-[34ch] text-[13px] text-ink-2">{text}</p>
    </div>
  )
}
