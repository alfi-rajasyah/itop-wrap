import { useCallback, useRef, type ClipboardEvent } from 'react'
import { Composer } from '@/components/kit'
import { useModel, type Composer as ComposerView, type ListScope } from '@/backend'
import { useModelText } from '@/lib/useModelText'
import { pickDropped, pickPasted, toIntake, type IntakePick } from './intake'

/**
 * The log composer pinned under the detail (LOG-1…23). Draft, images, labels, the status
 * line and every enabled state are the snapshot's; this wires typing to `setDraft`, images
 * from paste, drop and the paperclip to `addImages`, and ⌘↩ / the button to `send`.
 */
export function DetailComposer({ scope, ticketId, composer }: { scope: ListScope; ticketId: string; composer: ComposerView }) {
  const [, send] = useModel(`detail:${scope}`)
  const pickerRef = useRef<HTMLInputElement>(null)
  const draftKey = `${ticketId}|${composer.isPrivate ? 'private' : 'public'}`

  const commit = useCallback((text: string) => send({ type: 'setDraft', text }), [send])
  const [draft, setDraft] = useModelText(composer.draft, commit, draftKey)

  const add = (pick: IntakePick | null) => {
    if (!pick) return
    void toIntake(pick, composer.maxIntakeFileBytes)
      .then((items) => send({ type: 'addImages', items }))
      .catch((error: unknown) => console.error('addImages failed', error))
  }

  const onPaste = (e: ClipboardEvent<HTMLTextAreaElement>) => {
    if (!composer.editable) return
    const pick = pickPasted(e.clipboardData)
    if (!pick) return
    e.preventDefault()
    add(pick)
  }

  return (
    <div className="shrink-0 border-t border-line bg-[color-mix(in_srgb,var(--app-bg)_70%,var(--surface))] px-6 pb-4 pt-3">
      <div className="max-w-[860px]">
        <Composer
          value={draft}
          onChange={setDraft}
          onSend={() => void send({ type: 'send' }).catch((error: unknown) => console.error('send failed', error))}
          label={composer.accessibilityLabel}
          placeholder={composer.placeholder}
          sendLabel={composer.sendLabel}
          sendDisabled={!composer.canSend}
          sendTooltip={composer.sendHelp}
          busy={composer.sending}
          readOnly={!composer.editable}
          images={composer.images.map((im) => ({ id: im.id, src: im.thumbnailSrc, name: im.filename, sizeLabel: im.sizeText }))}
          onRemoveImage={(imageId) => void send({ type: 'removeImage', imageId })}
          onAttach={() => pickerRef.current?.click()}
          attachDisabled={!composer.canAddImages}
          attachTooltip={composer.paperclipHelp}
          status={composer.status?.text}
          statusTone={composer.status?.isError ? 'error' : 'info'}
          onPaste={onPaste}
          onDropFiles={(files) => add(pickDropped(files))}
        />
      </div>
      <input
        ref={pickerRef}
        type="file"
        accept="image/*"
        multiple
        hidden
        tabIndex={-1}
        aria-hidden
        onChange={(e) => {
          const files = Array.from(e.currentTarget.files ?? [])
          e.currentTarget.value = ''
          if (files.length) add({ files, raw: false })
        }}
      />
    </div>
  )
}
