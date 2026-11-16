import { useEffect, useRef, useState, type KeyboardEvent } from 'react'
import { createPortal } from 'react-dom'
import { Button, Checkbox, Tooltip } from '@/components/kit'

/**
 * The full-size image viewer (DET-49): sized to the image within 1100 × 720 (at least
 * 480 × 300, and never larger than the window), the image fitted; "Actual size" shows it 1:1
 * with scrolling; the pixel size; "Close" (Esc). Focus stays inside and returns on close.
 */
export function ImageViewer({ src, width, height, onClose }: { src: string; width: number; height: number; onClose: () => void }) {
  const [actual, setActual] = useState(false)
  const panelRef = useRef<HTMLDivElement>(null)
  const closeRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    const restore = document.activeElement as HTMLElement | null
    closeRef.current?.focus()
    return () => restore?.focus?.()
  }, [])

  const w = Math.max(width, 1)
  const h = Math.max(height, 1)
  const scale = Math.min(1, 1100 / w, 720 / h)
  const vw = typeof window !== 'undefined' ? window.innerWidth - 64 : 1100
  const vh = typeof window !== 'undefined' ? window.innerHeight - 120 : 720
  const frameW = Math.min(Math.max(480, w * scale), Math.max(240, vw))
  const frameH = Math.min(Math.max(300, h * scale), Math.max(160, vh))

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      onClose()
    } else if (e.key === 'Tab') {
      const items = Array.from(panelRef.current?.querySelectorAll<HTMLElement>('input, button, [tabindex="0"]') ?? [])
      if (!items.length) return
      const first = items[0]!
      const last = items[items.length - 1]!
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault()
        last.focus()
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault()
        first.focus()
      }
    }
  }

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div aria-hidden className="fixed inset-0 bg-[oklch(0.2_0.01_264/0.28)] dark:bg-[oklch(0_0_0/0.5)]" style={{ animation: 'fade-in 160ms ease-out both' }} />
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label="Image"
        onKeyDown={onKeyDown}
        className="relative flex flex-col gap-3 rounded-[14px] bg-surface p-4 text-ink shadow-overlay"
        style={{ animation: 'pop-in 180ms cubic-bezier(0.23,1,0.32,1) both' }}
      >
        <div
          tabIndex={actual ? 0 : -1}
          aria-label={actual ? 'Image at actual size' : undefined}
          className="kit-scroll flex items-center justify-center rounded-[8px] bg-inset"
          style={{ width: frameW, height: frameH, overflow: actual ? 'auto' : 'hidden' }}
        >
          {actual ? (
            <img src={src} alt="" className="m-auto block max-w-none" style={{ width, height }} draggable={false} />
          ) : (
            <img src={src} alt="" className="block max-h-full max-w-full object-contain" draggable={false} />
          )}
        </div>
        <div className="flex items-center gap-3">
          <Tooltip content="Show the image at 100%, scrolling if it's bigger than the window" describe>
            <span className="inline-flex">
              <Checkbox checked={actual} onChange={setActual} label="Actual size" />
            </span>
          </Tooltip>
          <span className="mono-figures text-[11.5px] text-ink-2">
            {width} × {height}
          </span>
          <span className="flex-1" />
          <Button ref={closeRef} size="sm" variant="secondary" onClick={onClose} aria-keyshortcuts="Escape">
            Close
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  )
}
