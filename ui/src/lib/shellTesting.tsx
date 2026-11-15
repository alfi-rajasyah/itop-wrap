/**
 * Test helpers for the screens: a fresh mock backend behind `useModel`, with `dispatch` spied on
 * so a test can check which intents an interaction sent. Imported by tests only.
 */
import { act, waitFor } from '@testing-library/react'
import { vi } from 'vitest'
import { ModelStore, setStoreForTests, type IntentOf, type ModelId, type SnapshotOf } from '@/backend'
import { MockBackend, type MockOptions } from '@/backend/mock'

const live: MockBackend[] = []

export function mountMock(options: Partial<MockOptions> = {}) {
  const mock = new MockBackend({ delayMs: 0, ...options })
  live.push(mock)
  const dispatch = vi.spyOn(mock, 'dispatch')
  setStoreForTests(new ModelStore(mock))
  /** Was this intent sent to this model (payload fields matched loosely)? */
  const sent = (model: ModelId, intent: Record<string, unknown>) =>
    dispatch.mock.calls.some(([m, i]) => m === model && Object.entries(intent).every(([k, v]) => JSON.stringify((i as Record<string, unknown>)[k]) === JSON.stringify(v)))
  return { mock, dispatch, sent }
}

export function disposeMocks() {
  live.splice(0).forEach((m) => m.dispose())
}

/** Waits until the mock's snapshot of `model` satisfies the predicate. */
export async function until<K extends ModelId>(mock: MockBackend, model: K, predicate: (s: SnapshotOf<K>) => boolean) {
  let last: SnapshotOf<K> | null = null
  await waitFor(async () => {
    last = await mock.getSnapshot(model)
    if (last === null || !predicate(last)) throw new Error(`${model} not ready: ${JSON.stringify(last)}`)
  })
  return last as unknown as SnapshotOf<K>
}

/** Sends an intent straight to the mock (as another window or Rust would) and lets React catch up. */
export async function backendDispatch<K extends ModelId>(mock: MockBackend, model: K, intent: IntentOf<K>) {
  let result: unknown
  await act(async () => {
    result = await mock.dispatch(model, intent)
  })
  return result
}
