/* Drops outside the places that take them do nothing. A web view's own reaction to an unhandled
 * drop is to open what was dropped (a link, a file) in place of the app; Rust refuses that
 * navigation too (`is_app_url`), this keeps it from being attempted. Text fields still take
 * dropped text, and a `[data-dropzone]` (the composer) handles its own drops. */

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false
  return target.closest('input, textarea, [contenteditable]:not([contenteditable="false"])') !== null
}

function inDropzone(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('[data-dropzone]') !== null
}

export function onDragOver(e: DragEvent) {
  if (e.defaultPrevented || isEditable(e.target) || inDropzone(e.target)) return
  e.preventDefault()
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'none'
}

export function onDrop(e: DragEvent) {
  if (e.defaultPrevented || isEditable(e.target)) return
  e.preventDefault()
}

let installed = false

export function installDropGuard(target: Window = window) {
  if (installed) return
  installed = true
  target.addEventListener('dragover', onDragOver)
  target.addEventListener('drop', onDrop)
}
