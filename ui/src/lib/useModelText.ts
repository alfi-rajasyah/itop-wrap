import { useCallback, useEffect, useReducer, useRef, useState } from 'react'

/**
 * A text field whose value is model state (contract §1 rule 4): every change is sent as an
 * intent and the snapshot's value is shown. The local copy only keeps typing responsive: while
 * our own changes are in flight the snapshot may still carry an older value, so it is adopted
 * only once nothing we sent is pending (a send that clears the draft, a ticket switch, a reset).
 *
 * `resetKey` names whose text this is (e.g. "<ticketId>|public"); a new key shows the
 * snapshot's value at once.
 */
export function useModelText(
  value: string,
  commit: (text: string) => Promise<unknown>,
  resetKey?: string,
): [string, (text: string) => void] {
  const [local, setLocal] = useState(value)
  const [key, setKey] = useState(resetKey)
  const inFlight = useRef(0)
  const [settled, bump] = useReducer((n: number) => n + 1, 0)

  if (key !== resetKey) {
    setKey(resetKey)
    setLocal(value)
  }

  useEffect(() => {
    if (inFlight.current === 0) setLocal(value)
  }, [value, settled])

  const change = useCallback(
    (text: string) => {
      setLocal(text)
      inFlight.current += 1
      commit(text)
        .catch((error: unknown) => console.error('text intent failed', error))
        .finally(() => {
          inFlight.current -= 1
          bump()
        })
    },
    [commit],
  )

  return [local, change]
}
