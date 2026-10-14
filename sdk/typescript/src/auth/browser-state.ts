/** Native storage visibility is not ordered by Web Locks. Commit the handoff. */
import { parseSession, type Session } from './session.js';
import { encodeJson } from '../core/json.js';
import { createVerifier } from './pkce.js';

type Receipt = { kind: 'ready' | 'refreshing'; owner: string; value: string | null };
type Snapshot = { value: string | null; retainDirty: boolean };
const DATABASE = 'nelcota.auth';
const STORE = 'sessions';
const DEADLINE = 5000;

export class BrowserSessionConflictError extends Error {
  constructor() { super('Browser session changed during refresh'); }
}

function receipt(value: unknown): Receipt {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) throw new Error('Invalid browser session receipt');
  const fields = value as Record<string, unknown>;
  if ((fields['kind'] !== 'ready' && fields['kind'] !== 'refreshing') || typeof fields['owner'] !== 'string' || fields['owner'].length === 0 || fields['owner'].length > 128 || !(fields['value'] === null || typeof fields['value'] === 'string')) throw new Error('Invalid browser session receipt');
  return value as Receipt;
}

export class BrowserSessionState {
  readonly #owner = createVerifier();
  constructor(readonly key: string, private readonly factory: IDBFactory | undefined) {}

  async #open(): Promise<IDBDatabase> {
    if (!this.factory) throw new Error('IndexedDB is required for coordinated browser sessions');
    return new Promise((resolve, reject) => {
      const request = this.factory!.open(DATABASE, 1);
      let settled = false;
      const fail = (error: unknown): void => {
        if (settled) return;
        settled = true; clearTimeout(timer); reject(error);
      };
      const timer = setTimeout(() => fail(new Error('Browser session storage timed out')), DEADLINE);
      request.onblocked = () => fail(new Error('Browser session storage is blocked'));
      request.onerror = () => fail(request.error ?? new Error('Browser session storage failed'));
      request.onupgradeneeded = () => {
        if (settled) { request.transaction?.abort(); return; }
        request.result.createObjectStore(STORE);
      };
      request.onsuccess = () => {
        if (settled) { request.result.close(); return; }
        if (!request.result.objectStoreNames.contains(STORE)) {
          request.result.close(); fail(new Error('Invalid browser session database')); return;
        }
        settled = true; clearTimeout(timer); resolve(request.result);
      };
    });
  }

  async #transaction<T>(operation: (store: IDBObjectStore, previous: unknown) => T): Promise<T> {
    const database = await this.#open();
    return new Promise((resolve, reject) => {
      let transaction: IDBTransaction;
      try { transaction = database.transaction(STORE, 'readwrite', { durability: 'strict' }); }
      catch (error) { database.close(); reject(error); return; }
      let value: T;
      let failure: unknown;
      const timer = setTimeout(() => {
        failure = new Error('Browser session transaction timed out');
        try { transaction.abort(); }
        catch { database.close(); reject(failure); }
      }, DEADLINE);
      transaction.oncomplete = () => { clearTimeout(timer); database.close(); resolve(value); };
      transaction.onabort = () => { clearTimeout(timer); database.close(); reject(failure ?? transaction.error ?? new Error('Browser session transaction aborted')); };
      try {
        const store = transaction.objectStore(STORE);
        const request = store.get(this.key);
        request.onsuccess = () => {
          try { value = operation(store, request.result); }
          catch (error) { failure = error; transaction.abort(); }
        };
      }
      catch (error) { failure = error; transaction.abort(); }
    });
  }

  read(legacy: () => string | null, dirty?: string | null, allowPending = false): Promise<Snapshot> {
    return this.#transaction((store, previous) => {
      if (previous === undefined) {
        const value = dirty !== undefined ? dirty : legacy();
        store.put({ kind: 'ready', owner: this.#owner, value } satisfies Receipt, this.key);
        return { value, retainDirty: dirty !== undefined };
      }
      const state = receipt(previous);
      const owned = state.owner === this.#owner;
      if (state.kind === 'refreshing' && !owned && !allowPending) throw new Error('Browser session refresh was interrupted; sign in again');
      return { value: owned && dirty !== undefined ? dirty : state.value, retainDirty: owned && dirty !== undefined };
    });
  }

  reserve(session: Session): Promise<void> {
    return this.#transaction((store, previous) => {
      const state = receipt(previous);
      if (state.kind === 'refreshing' && state.owner !== this.#owner) throw new Error('Browser session refresh was interrupted; sign in again');
      if (state.kind === 'ready' && parseSession(state.value)?.refresh_token !== session.refresh_token) throw new Error('Browser session changed before refresh');
      store.put({ kind: 'refreshing', owner: this.#owner, value: encodeJson(session) } satisfies Receipt, this.key);
    });
  }

  write(value: string | null, completingRefresh = false): Promise<void> {
    return this.#transaction((store, previous) => {
      if (completingRefresh) {
        const state = receipt(previous);
        if (state.kind !== 'refreshing' || state.owner !== this.#owner) throw new BrowserSessionConflictError();
      }
      store.put({ kind: 'ready', owner: this.#owner, value } satisfies Receipt, this.key);
    });
  }
}
