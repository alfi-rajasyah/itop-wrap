import { useEffect, useRef, useState } from 'react'

/**
 * A text field whose value the Rust model owns (contract §1.4). React keeps a local copy while
 * typing so keystrokes never wait for the round trip, sends every change as an intent, and adopts
 * the snapshot's value when Rust changes it on its own (e.g. `openTicket` puts a ref in the search,
 * a menu bar open clears it). Echoes of values we sent, which can arrive late and out of step with
 * the typing, are ignored.
 */
export function useSyncedText(snapshotValue: string, send: (value: string) => unknown): [string, (value: string) => void] {
  const [local, setLocal] = useState(snapshotValue)
  const pending = useRef<string[]>([])

  useEffect(() => {
    const i = pending.current.indexOf(snapshotValue)
    if (i >= 0) {
      // Our own value coming back: drop it and everything sent before it.
      pending.current = pending.current.slice(i + 1)
      return
    }
    pending.current = []
    setLocal(snapshotValue)
  }, [snapshotValue])

  const change = (value: string) => {
    setLocal(value)
    pending.current.push(value)
    void send(value)
  }

  return [local, change]
}
