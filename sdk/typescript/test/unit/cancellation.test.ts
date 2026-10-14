import { expect, it, vi } from 'vitest';
import { abortable } from '../../src/core/cancellation.js';

it('cancels shared waiters independently and removes their listeners immediately', async () => {
  let finish!: (value: string) => void;
  const operation = new Promise<string>(resolve => { finish = resolve; });
  const cancelled = new AbortController(), active = new AbortController();
  const remove = vi.spyOn(cancelled.signal, 'removeEventListener');
  const first = abortable(operation, cancelled.signal);
  const second = abortable(operation, active.signal);
  cancelled.abort();
  expect((await first).error?.code).toBe('aborted');
  expect(remove).toHaveBeenCalledWith('abort', expect.any(Function));
  finish('accepted');
  expect((await second).data).toBe('accepted');
  expect((await abortable(operation, new AbortController().signal)).data).toBe('accepted');
});

it('preserves rejection for active callers after another caller cancels', async () => {
  let fail!: (error: unknown) => void;
  const operation = new Promise<string>((_resolve, reject) => { fail = reject; });
  const cancelled = new AbortController();
  const first = abortable(operation, cancelled.signal);
  const second = abortable(operation, new AbortController().signal);
  const rejected = expect(second).rejects.toBeUndefined();
  cancelled.abort();
  expect((await first).error?.code).toBe('aborted');
  fail(undefined);
  await rejected;
});

it('observes a late rejection even when every caller starts already aborted', async () => {
  let fail!: (error: unknown) => void;
  const operation = new Promise<string>((_resolve, reject) => { fail = reject; });
  const controller = new AbortController();
  controller.abort();
  expect((await abortable(operation, controller.signal)).error?.code).toBe('aborted');
  fail(new Error('late failure'));
  await new Promise(resolve => setTimeout(resolve, 0));
});
