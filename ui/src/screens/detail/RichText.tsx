import { useState } from 'react'
import { ErrorIcon, ImageIcon, Spinner, Tooltip } from '@/components/kit'
import { openUrl, type RichBlock, type RichContent, type RichRun } from '@/backend'
import { cn } from '@/lib/utils'
import { ImageViewer } from './ImageViewer'

/* ─────────────────────────────────────────────────────────
 * RICH TEXT (DET-40…50). Rust has already parsed the HTML into
 * text runs and image blocks; this only draws them:
 *  - text keeps its line breaks, links (http/https/mailto) open in
 *    the default app through `openUrl`;
 *  - a small outside image is a grey photo glyph, never loaded;
 *  - a larger outside image is a chip "[photo] host ↗" that opens it;
 *  - an image block loads its `src` (itop:// asset or data: URL),
 *    ≤ 640 wide (never upscaled), ≤ 480 high, and opens the viewer.
 * Lines are capped at a comfortable measure for long entries.
 * ───────────────────────────────────────────────────────── */

export function RichText({ content, className }: { content: RichContent; className?: string }) {
  return (
    <div className={cn('flex flex-col gap-2 text-[13px] leading-[1.6] text-ink', className)}>
      {content.blocks.map((block, i) => (
        <Block key={i} block={block} />
      ))}
    </div>
  )
}

function Block({ block }: { block: RichBlock }) {
  if (block.type === 'image') return <RichImage src={block.src} ticketWebUrl={block.ticketWebUrl} />
  return (
    <p className="max-w-[72ch] select-text whitespace-pre-wrap [overflow-wrap:anywhere]">
      {block.runs.map((run, i) => (
        <Run key={i} run={run} />
      ))}
    </p>
  )
}

function open(url: string) {
  openUrl(url).catch((error: unknown) => console.error('openUrl failed', error))
}

function Run({ run }: { run: RichRun }) {
  switch (run.type) {
    case 'text':
      return run.link ? (
        <a
          href={run.link}
          title={run.link}
          onClick={(e) => {
            e.preventDefault()
            open(run.link!)
          }}
          className="rounded-[3px] text-ink underline decoration-ink-3 underline-offset-[3px] hover:decoration-ink"
        >
          {run.text}
        </a>
      ) : (
        <>{run.text}</>
      )
    case 'icon':
      return <ImageIcon role="img" aria-label="Image" className="inline size-[1.05em] align-[-0.18em] text-ink-2" />
    case 'outsideImage':
      return (
        <a
          href={run.url}
          title={run.url}
          onClick={(e) => {
            e.preventDefault()
            open(run.url)
          }}
          className="mx-0.5 inline-flex items-center gap-1 whitespace-nowrap rounded-[6px] bg-field px-1.5 align-baseline text-[12px] leading-[20px] text-ink-2 shadow-hairline hover:text-ink"
        >
          <ImageIcon aria-hidden className="size-3.5 shrink-0" />
          {run.host} ↗
        </a>
      )
  }
}

/** One loaded image: spinner while loading, the failure line with the web link, or the picture. */
export function RichImage({ src, ticketWebUrl }: { src: string; ticketWebUrl: string | null }) {
  const [state, setState] = useState<'loading' | 'loaded' | 'failed'>('loading')
  const [size, setSize] = useState<{ w: number; h: number } | null>(null)
  const [viewing, setViewing] = useState(false)
  const [shown, setShown] = useState(src)

  if (shown !== src) {
    setShown(src)
    setState('loading')
    setSize(null)
  }

  if (state === 'failed') {
    return (
      <div role="alert" className="flex flex-wrap items-center gap-2 text-[12px] text-ink-2">
        <ErrorIcon aria-hidden className="size-4 shrink-0" />
        <span>Couldn&apos;t load this image.</span>
        {ticketWebUrl ? (
          <a
            href={ticketWebUrl}
            onClick={(e) => {
              e.preventDefault()
              open(ticketWebUrl)
            }}
            className="text-ink underline decoration-ink-3 underline-offset-[3px] hover:decoration-ink"
          >
            Open in iTop on the web
          </a>
        ) : null}
      </div>
    )
  }

  return (
    <div>
      {state === 'loading' ? (
        <div role="status" className="flex items-center gap-2 text-[12px] text-ink-2">
          <Spinner size={13} />
          Loading image
        </div>
      ) : null}
      <Tooltip content="Open full size" describe={false} className="flex max-w-full">
        <button
          type="button"
          aria-label="Image, open full size"
          onClick={() => setViewing(true)}
          className={cn('block max-w-full rounded-[6px]', state !== 'loaded' && 'hidden')}
        >
          <img
            src={src}
            alt=""
            draggable={false}
            onLoad={(e) => {
              setSize({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })
              setState('loaded')
            }}
            onError={() => setState('failed')}
            className="block h-auto w-auto rounded-[6px] object-contain shadow-hairline"
            style={{ maxWidth: `min(${Math.min(size?.w || 640, 640)}px, 100%)`, maxHeight: 480 }}
          />
        </button>
      </Tooltip>
      {viewing && size ? <ImageViewer src={src} width={size.w} height={size.h} onClose={() => setViewing(false)} /> : null}
    </div>
  )
}
