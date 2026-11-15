import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { useSyncedText } from './useSyncedText'

describe('useSyncedText', () => {
  it('sends every change, ignores late echoes of what it sent, and adopts values Rust sets itself', () => {
    const send = vi.fn()
    const { result, rerender } = renderHook(({ value }) => useSyncedText(value, send), { initialProps: { value: '' } })
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
})
