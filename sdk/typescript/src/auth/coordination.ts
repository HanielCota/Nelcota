/**
 * Coordination between tabs of the same app. The server ends a session when a
 * refresh token is used twice (reuse detection), so two tabs refreshing the
 * same session at once would sign the person out: refreshes take a Web Lock,
 * and each change is broadcast so other tabs drop their cached copy.
 */

interface LockManagerLike {
  request<T>(name: string, callback: () => Promise<T>): Promise<T>;
}

function locks(): LockManagerLike | undefined {
  const nav = (globalThis as { navigator?: { locks?: LockManagerLike } }).navigator;
  return nav?.locks;
}

/** Runs `task` holding a lock shared by every tab of this origin (when available). */
export function withLock<T>(name: string, task: () => Promise<T>): Promise<T> {
  const manager = locks();
  return manager ? manager.request(name, task) : task();
}

export type SessionSignal = 'changed';

export class Broadcast {
  readonly #channel: BroadcastChannel | undefined;

  constructor(name: string, onMessage: () => void) {
    if (typeof BroadcastChannel === 'undefined') return;
    this.#channel = new BroadcastChannel(name);
    this.#channel.onmessage = (event: MessageEvent<unknown>) => {
      if (event.data === 'changed') onMessage();
    };
    // Node keeps the process alive while a channel is open.
    (this.#channel as { unref?: () => void }).unref?.();
  }

  notify(): void {
    this.#channel?.postMessage('changed' satisfies SessionSignal);
  }

  close(): void {
    this.#channel?.close();
  }
}
