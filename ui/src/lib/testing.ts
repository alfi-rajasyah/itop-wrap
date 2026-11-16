/* Test helpers for the screens: a fresh mock backend behind `useModel`, waiting for a model to
 * reach a state, selecting a ticket, and a static backend for rendering exact snapshots. */
import { act } from '@testing-library/react'
import { ModelStore, setStoreForTests, type Backend, type ListScope, type ModelId, type SnapshotOf } from '@/backend'
import { MockBackend, type MockOptions } from '@/backend/mock'

const live: MockBackend[] = []

/** A mock with no latency, wired into `useModel`. Disposed by `disposeMocks`. */
export function startMock(options: Partial<MockOptions> = {}): MockBackend {
  const mock = new MockBackend({ delayMs: 0, ...options })
  live.push(mock)
  setStoreForTests(new ModelStore(mock))
  return mock
}

export function disposeMocks() {
  live.splice(0).forEach((m) => m.dispose())
}

/** Polls the model until the predicate holds (background loads finish after dispatch returns). */
export async function until<K extends ModelId>(
  backend: Backend,
  model: K,
  predicate: (s: SnapshotOf<K>) => boolean,
): Promise<SnapshotOf<K>> {
  for (let i = 0; i < 300; i++) {
    const s = await backend.getSnapshot(model)
    if (s !== null && predicate(s)) return s
    await act(() => new Promise((r) => setTimeout(r, 2)))
  }
  throw new Error(`${model} never reached the expected state: ${JSON.stringify(await backend.getSnapshot(model))}`)
}

/** Waits for the list, selects the ticket with this ref and waits for its detail to load. */
export async function selectTicket(mock: MockBackend, scope: ListScope, ref: string) {
  const list = await until(mock, `list:${scope}`, (s) => s.content.type === 'list')
  if (list.content.type !== 'list') throw new Error('list not loaded')
  const row = list.content.body.sections.flatMap((s) => s.rows).find((r) => r.ticket.ref === ref)
  if (!row) throw new Error(`${ref} is not in list:${scope}`)
  await act(() => mock.dispatch(`list:${scope}`, { type: 'select', ticketId: row.ticket.id }))
  await until(mock, `detail:${scope}`, (s) => s.type === 'ticket' && s.body.type === 'loaded' && s.attachments.type !== 'loading')
  return row.ticket
}

/**
 * A backend that serves fixed snapshots and records intents, for rendering one exact state.
 * Unknown models are null.
 */
export function staticBackend(snapshots: Partial<{ [K in ModelId]: SnapshotOf<K> }>) {
  const sent: { model: ModelId; intent: unknown }[] = []
  const opened: string[] = []
  const backend: Backend = {
    kind: 'mock',
    getSnapshot: async (model) => (snapshots[model] ?? null) as never,
    subscribe: () => () => {},
    dispatch: async (model, intent) => {
      sent.push({ model, intent })
      return null as never
    },
    native: {
      openUrl: async (url: string) => {
        opened.push(url)
      },
    },
  }
  setStoreForTests(new ModelStore(backend))
  return { backend, sent, opened }
}
