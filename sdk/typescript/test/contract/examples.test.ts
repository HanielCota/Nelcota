import { describe, expect, it } from 'vitest';
import { randomUUID } from 'node:crypto';
import { createClient } from '../../src/index.js';
import { URL_, signedIn, skip } from './setup.js';
import { runExample } from '../../examples/node.mjs';
import { createExampleServer } from '../../examples/server.mjs';

describe.skipIf(skip)('published examples against the server', () => {
  it('runs the Node signup, CRUD and upload/download example', async () => {
    const result = await runExample({ url: URL_, email: `demo-${randomUUID()}@example.com`, password: 'node-example-password', signUp: true });
    expect(result.note.body).toBe('Hello from Node');
    expect(result.downloaded).toBe('Hello from Node');
  });

  it('keeps concurrent server requests isolated by each caller token', async () => {
    const a = await signedIn();
    const b = await signedIn();
    await a.client.from('notes').insert({ body: 'only A' });
    await b.client.from('notes').insert({ body: 'only B' });
    const tokenA = (await a.client.auth.getSession()).data!.access_token;
    const tokenB = (await b.client.auth.getSession()).data!.access_token;
    const server = createExampleServer(URL_);
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
    const address = server.address();
    if (!address || typeof address === 'string') throw new Error('Missing server port');
    const url = `http://127.0.0.1:${address.port}/notes`;
    try {
      const responses = await Promise.all([tokenA, tokenB, tokenA, tokenB].map(async (token) => {
        const response = await fetch(url, { headers: { authorization: `Bearer ${token}` } });
        expect(response.status).toBe(200);
        return response.json();
      }));
      expect(responses.map((rows) => rows.map((row: { body: string }) => row.body))).toEqual([['only A'], ['only B'], ['only A'], ['only B']]);
      expect((await fetch(url)).status).toBe(401);
      expect((await fetch(url, { headers: { authorization: 'Bearer invalid' } })).status).toBe(401);
      const visitor = await createClient(URL_).from('notes').select();
      expect(visitor.data).toBeNull();
      expect(visitor.error?.status).toBe(401);
    } finally {
      await new Promise<void>((resolve, reject) => server.close((error: Error | undefined) => error ? reject(error) : resolve()));
      a.client.dispose(); b.client.dispose();
    }
  });
});
