import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { isAppError, type IntentOf, type ModelId, type ResultOf, type SnapshotEvent, type SnapshotOf } from './contract'
import { BackendError, type Backend } from './transport'

/** Rust command and event names (docs/rewrite/contract.md, "Transport"). */
export const COMMANDS = {
  getSnapshot: 'get_snapshot',
  dispatch: 'dispatch',
  openUrl: 'open_url',
} as const

export const SNAPSHOT_EVENT = 'snapshot'

function toBackendError(raw: unknown): BackendError {
  if (isAppError(raw)) return new BackendError(raw)
  return new BackendError({ type: 'internal', message: raw instanceof Error ? raw.message : String(raw) })
}

async function call<T>(command: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (raw) {
    throw toBackendError(raw)
  }
}

/** The real backend: Tauri commands plus one `snapshot` event listener fanned out per model. */
export function createTauriBackend(): Backend {
  const subscribers = new Map<ModelId, Set<(snapshot: unknown) => void>>()
  let listening: Promise<unknown> | null = null

  function ensureListening() {
    listening ??= listen<SnapshotEvent>(SNAPSHOT_EVENT, (event) => {
      const { model, snapshot } = event.payload
      subscribers.get(model)?.forEach((callback) => callback(snapshot))
    })
  }

  return {
    kind: 'tauri',
    getSnapshot<K extends ModelId>(model: K) {
      return call<SnapshotOf<K> | null>(COMMANDS.getSnapshot, { model })
    },
    subscribe<K extends ModelId>(model: K, callback: (snapshot: SnapshotOf<K> | null) => void) {
      ensureListening()
      const set = subscribers.get(model) ?? new Set()
      const wrapped = callback as (snapshot: unknown) => void
      set.add(wrapped)
      subscribers.set(model, set)
      return () => {
        set.delete(wrapped)
      }
    },
    dispatch<K extends ModelId, I extends IntentOf<K>>(model: K, intent: I) {
      return call<ResultOf<K, I['type']>>(COMMANDS.dispatch, { model, intent })
    },
    native: {
      openUrl: (url: string) => call<void>(COMMANDS.openUrl, { url }),
    },
  }
}
