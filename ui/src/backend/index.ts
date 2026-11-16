import { useCallback, useSyncExternalStore } from 'react'
import type { ImageIntakeItem, IntentOf, ModelId, ResultOf, SnapshotOf } from './contract'
import { createMockBackend, mockOptionsFromSearch } from './mock'
import { createTauriBackend } from './tauri'
import type { Backend } from './transport'

export * from './contract'
export { BackendError, type Backend } from './transport'

/** True inside the Tauri web view. */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/**
 * Keeps the latest snapshot of each model in memory so React can read it synchronously
 * (useSyncExternalStore), fetching it once and then following the backend's pushes.
 */
export class ModelStore {
  readonly backend: Backend
  private readonly values = new Map<ModelId, unknown>()
  private readonly listeners = new Map<ModelId, Set<() => void>>()
  private readonly unsubscribe = new Map<ModelId, () => void>()

  constructor(backend: Backend) {
    this.backend = backend
  }

  /** The cached snapshot, or null before the first one arrived (or while the model doesn't exist). */
  read<K extends ModelId>(model: K): SnapshotOf<K> | null {
    return (this.values.get(model) as SnapshotOf<K> | undefined) ?? null
  }

  watch(model: ModelId, listener: () => void): () => void {
    const set = this.listeners.get(model) ?? new Set()
    set.add(listener)
    this.listeners.set(model, set)
    if (!this.unsubscribe.has(model)) this.follow(model)
    return () => {
      set.delete(listener)
      if (set.size === 0) {
        this.unsubscribe.get(model)?.()
        this.unsubscribe.delete(model)
        this.listeners.delete(model)
        this.values.delete(model)
      }
    }
  }

  private follow(model: ModelId) {
    let pushed = false
    this.unsubscribe.set(
      model,
      this.backend.subscribe(model, (snapshot) => {
        pushed = true
        this.set(model, snapshot)
      }),
    )
    // A push that arrives before the first read wins: it's newer.
    this.backend
      .getSnapshot(model)
      .then((snapshot) => {
        if (!pushed) this.set(model, snapshot)
      })
      .catch((error: unknown) => console.error(`getSnapshot(${model}) failed`, error))
  }

  private set(model: ModelId, snapshot: unknown) {
    this.values.set(model, snapshot)
    this.listeners.get(model)?.forEach((listener) => listener())
  }

  /**
   * Sends an intent. A model's `set…` intents (text fields, tabs: quick state changes) go out one
   * after another, and any later intent to the same model waits until they have been applied, so
   * a confirm, create or send issued right after typing can never overtake the last keystroke
   * (Tauri runs async commands concurrently). Other intents don't wait for each other.
   */
  dispatch<K extends ModelId, I extends IntentOf<K>>(model: K, intent: I): Promise<ResultOf<K, I['type']>> {
    const before = this.settingQueue.get(model)
    const run = () => this.backend.dispatch(model, intent)
    const result = before ? before.then(run) : run()
    if (intent.type.startsWith('set')) {
      const settled = result.then(
        () => undefined,
        () => undefined,
      )
      this.settingQueue.set(model, settled)
      void settled.then(() => {
        if (this.settingQueue.get(model) === settled) this.settingQueue.delete(model)
      })
    }
    return result
  }

  /** Per model: when its last `set…` intent has been applied. */
  private readonly settingQueue = new Map<ModelId, Promise<void>>()
}

function selectBackend(): Backend {
  if (isTauri()) return createTauriBackend()
  const search = typeof window !== 'undefined' ? window.location.search : ''
  return createMockBackend(mockOptionsFromSearch(search))
}

/** The app's backend: Tauri inside the app, the mock in a plain browser and in tests. */
export const backend: Backend = selectBackend()

let store = new ModelStore(backend)

/** For tests: point the hook at another backend (e.g. a fresh mock). */
export function setStoreForTests(next: ModelStore) {
  store = next
}

export function getStore(): ModelStore {
  return store
}

export type Send<K extends ModelId> = <I extends IntentOf<K>>(intent: I) => Promise<ResultOf<K, I['type']>>

/**
 * `const [list, send] = useModel('list:mine')`. The snapshot is null until it has arrived, and for
 * workspace models while signed out.
 */
export function useModel<K extends ModelId>(model: K): [SnapshotOf<K> | null, Send<K>] {
  const current = store
  const snapshot = useSyncExternalStore(
    useCallback((listener: () => void) => current.watch(model, listener), [current, model]),
    () => current.read(model),
  )
  const send = useCallback(
    <I extends IntentOf<K>>(intent: I) => current.dispatch<K, I>(model, intent),
    [current, model],
  ) as Send<K>
  return [snapshot, send]
}

/** Opens an http, https or mailto URL in the default app. */
export function openUrl(url: string): Promise<void> {
  return store.backend.native.openUrl(url)
}

function toBase64(bytes: Uint8Array): string {
  let binary = ''
  const chunk = 0x8000
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk))
  }
  return btoa(binary)
}

/**
 * Turns a pasted, dropped or picked file into what `detail.addImages` takes. Files over
 * `maxBytes` (the composer's `maxIntakeFileBytes`) aren't read; Rust reports them as too large.
 */
export async function fileToIntake(file: File, maxBytes: number): Promise<ImageIntakeItem> {
  if (file.size > maxBytes) return { filename: file.name, dataBase64: null, size: file.size }
  const bytes = new Uint8Array(await file.arrayBuffer())
  return { filename: file.name, dataBase64: toBase64(bytes), size: file.size }
}
