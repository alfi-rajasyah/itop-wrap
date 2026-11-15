import { useModel } from '@/backend'
import { NoticeBanner, StopwatchIcon } from '@/components/kit'

/**
 * The timer's notice (TMR-20): stopwatch, the text, "Retry" when the save can be retried
 * (disabled while saving, TMR-12), and × to dismiss or discard (TMR-11). Shown above the lists
 * (LIST-69) and in the menu bar panel (MB-9).
 */
export function TimerNotice({ className }: { className?: string }) {
  const [timer, send] = useModel('timer')
  const notice = timer?.notice
  if (!timer || !notice) return null
  return (
    <NoticeBanner
      tone={notice.isError ? 'error' : 'info'}
      icon={<StopwatchIcon />}
      onRetry={notice.canRetry ? () => void send({ type: 'retry' }) : undefined}
      retryDisabled={timer.saving}
      onDismiss={() => void send({ type: 'clearNotice' })}
      dismissLabel={notice.dismissLabel}
      className={className}
    >
      {notice.text}
    </NoticeBanner>
  )
}
