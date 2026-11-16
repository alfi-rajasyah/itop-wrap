import { useCallback, useEffect, useRef, useState } from 'react'

/**
 * A text field whose value the Rust model owns (contract §1 rule 4). React keeps a local copy so
 * keystrokes never wait for the round trip, sends every change as an intent, and adopts the
 * snapshot's value when Rust changes it on its own (a ref put in the search by `openTicket`, the
 * draft cleared after a send, a sheet reset). Echoes of values we sent, which can arrive late and
 * out of step with the typing, are ignored.
 *
 * Ordering: the transport sends a model's `set…` intents in order and holds any later intent to
 * the same model until they have been applied (`ModelStore.dispatch`), so a confirm, create or
 * send right after typing always sees the last keystroke.
 *
 * `resetKey` names whose text this is (e.g. "<ticketId>|public"); a new key shows the snapshot's
 * value at once.
 */
export function useModelText(
  value: string,
  commit: (text: string) => unknown,
  resetKey?: string,
): [string, (text: string) => void] {
  const [local, setLocal] = useState(value)
  const [key, setKey] = useState(resetKey)
  const pending = useRef<string[]>([])

  if (key !== resetKey) {
    setKey(resetKey)
    setLocal(value)
    pending.current = []
  }

  useEffect(() => {
    const i = pending.current.indexOf(value)
    if (i >= 0) {
      // Our own value coming back: drop it and everything sent before it.
      pending.current = pending.current.slice(i + 1)
      return
    }
    pending.current = []
    setLocal(value)
  }, [value])

  const change = useCallback(
    (text: string) => {
      setLocal(text)
      pending.current.push(text)
      Promise.resolve(commit(text)).catch((error: unknown) => console.error('text intent failed', error))
    },
    [commit],
  )

  return [local, change]
}
