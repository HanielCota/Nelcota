// The built SDK must release cancelled callers before a shared rotation settles.
import assert from 'node:assert/strict';
import { setImmediate } from 'node:timers/promises';
import { abortable } from '../../dist/core/cancellation.js';

assert.equal(typeof globalThis.gc, 'function', 'Run this check with node --expose-gc');

async function retained(references) {
  // WeakRef targets survive their creation job. Collect across separate turns.
  for (let index = 0; index < 10; index++) {
    await setImmediate();
    globalThis.gc();
  }
  return references.filter(reference => reference.deref() !== undefined).length;
}

async function round(number) {
  let finish;
  const operation = new Promise(resolve => { finish = resolve; });
  const active = abortable(operation, new AbortController().signal);
  async function cancel() {
    const controller = new AbortController();
    const reference = new WeakRef(controller.signal);
    const pending = abortable(operation, controller.signal);
    controller.abort();
    assert.equal((await pending).error?.code, 'aborted');
    return reference;
  }
  const references = [];
  for (let index = 0; index < 1000; index++) references.push(await cancel());
  const count = await retained(references);
  assert.equal(count, 0, 'A pending shared operation retained cancelled callers');
  finish('accepted');
  assert.equal((await active).data, 'accepted');
  console.log(JSON.stringify({ round: number, cancelledCallers: references.length, retainedSignalsBeforeCompletion: count }));
  return new WeakRef(operation);
}

const operations = [];
for (let number = 1; number <= 3; number++) operations.push(await round(number));
assert.equal(await retained(operations), 0, 'Completed operations remained rooted');
