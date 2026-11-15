import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ModelStore } from '@/backend'
import type { Backend } from '@/backend/transport'
import { useModelText } from './useModelText'

describe('useModelText', () => {
  it('sends every change, ignores late echoes of what it sent, and adopts values Rust sets itself', () => {
    const send = vi.fn()
    const { result, rerender } = renderHook(({ value }) => useModelText(value, send), { initialProps: { value: '' } })
    act(() => result.current[1]('a'))
    act(() => result.current[1]('ab'))
    expect(send.mock.calls.map((c) => c[0])).toEqual(['a', 'ab'])
    // The echo of "a" arrives after "ab" was typed: the field keeps "ab".
    rerender({ value: 'a' })
    expect(result.current[0]).toBe('ab')
    rerender({ value: 'ab' })
    expect(result.current[0]).toBe('ab')
    // Rust puts a ref in the search (openTicket): the field takes it.
    rerender({ value: 'R-000512' })
    expect(result.current[0]).toBe('R-000512')
  })

  it('shows the snapshot value at once for a new reset key', () => {
    const send = vi.fn()
    const { result, rerender } = renderHook(({ value, k }) => useModelText(value, send, k), {
      initialProps: { value: 'draft one', k: 'a' },
    })
    act(() => result.current[1]('draft one!'))
    rerender({ value: 'other ticket', k: 'b' })
    expect(result.current[0]).toBe('other ticket')
  })
})

describe('ModelStore ordering (typing race)', () => {
  it('a confirm issued right after typing reaches the backend only after the last text intent', async () => {
    const received: string[] = []
    const releases: (() => void)[] = []
    const backend: Backend = {
      kind: 'mock',
      getSnapshot: () => Promise.resolve(null),
      subscribe: () => () => {},
      native: { openUrl: () => Promise.resolve() },
      dispatch: ((_model: string, intent: { type: string; reason?: string }) => {
        received.push(intent.reason !== undefined ? `${intent.type}:${intent.reason}` : intent.type)
        // Text intents take a while (the async command runs on another worker).
        if (intent.type.startsWith('set')) return new Promise<null>((resolve) => releases.push(() => resolve(null)))
        return Promise.resolve(null)
      }) as Backend['dispatch'],
    }
    const store = new ModelStore(backend)
    const { result } = renderHook(() =>
      useModelText('', (reason) => store.dispatch('pending:mine', { type: 'setReason', reason })),
    )
    act(() => result.current[1]('W'))
    act(() => result.current[1]('Wa'))
    const confirmed = store.dispatch('pending:mine', { type: 'confirm' })
    await Promise.resolve()
    expect(received).toEqual(['setReason:W'])
    releases.shift()?.()
    await vi.waitFor(() => expect(received).toEqual(['setReason:W', 'setReason:Wa']))
    expect(received).not.toContain('confirm')
    releases.shift()?.()
    await confirmed
    expect(received).toEqual(['setReason:W', 'setReason:Wa', 'confirm'])
  })
})
