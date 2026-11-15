import type { AppError, IntentOf, ModelId, NativeApi, ResultOf, SnapshotOf } from './contract'

/**
 * The generic transport both implementations (Tauri and the in-memory mock) provide.
 * See docs/rewrite/contract.md, "Transport".
 */
export interface Backend {
  readonly kind: 'tauri' | 'mock'
  /** The model's current snapshot; null when the model doesn't exist (a workspace model while signed out). */
  getSnapshot<K extends ModelId>(model: K): Promise<SnapshotOf<K> | null>
  /** Called with every new snapshot Rust emits for the model. Returns the unsubscribe function. */
  subscribe<K extends ModelId>(model: K, callback: (snapshot: SnapshotOf<K> | null) => void): () => void
  /** Sends one intent; resolves with its result, rejects with a BackendError. */
  dispatch<K extends ModelId, I extends IntentOf<K>>(model: K, intent: I): Promise<ResultOf<K, I['type']>>
  readonly native: NativeApi
}

/** A transport-level failure (not a business failure: those are results or snapshot fields). */
export class BackendError extends Error {
  readonly error: AppError

  constructor(error: AppError) {
    super(error.message)
    this.name = 'BackendError'
    this.error = error
  }
}
