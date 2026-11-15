import { onDragOver, onDrop } from './dropGuard'

function event(type: string, target: Element, prevented = false) {
  const e = new Event(type, { bubbles: true, cancelable: true }) as DragEvent
  Object.defineProperty(e, 'target', { value: target })
  if (prevented) e.preventDefault()
  return e
}

describe('drop guard', () => {
  it('cancels drops on the page so the web view never opens what was dropped', () => {
    const div = document.createElement('div')
    const over = event('dragover', div)
    onDragOver(over)
    expect(over.defaultPrevented).toBe(true)
    const drop = event('drop', div)
    onDrop(drop)
    expect(drop.defaultPrevented).toBe(true)
  })

  it('leaves text fields and the composer dropzone alone', () => {
    const input = document.createElement('textarea')
    const zone = document.createElement('div')
    zone.setAttribute('data-dropzone', '')
    const inner = document.createElement('span')
    zone.appendChild(inner)
    for (const t of [input, inner]) {
      const over = event('dragover', t)
      onDragOver(over)
      expect(over.defaultPrevented).toBe(false)
    }
    const drop = event('drop', input)
    onDrop(drop)
    expect(drop.defaultPrevented).toBe(false)
  })
})
