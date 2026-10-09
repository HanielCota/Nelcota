import { describe, expect, it } from 'vitest';
import { createClient, NelcotaUsageError } from '../../src/index.js';
import { empty, json, mockFetch } from './helpers.js';

function client(...replies: Parameters<typeof mockFetch>) {
  const mock = mockFetch(...replies);
  return { nelcota: createClient('https://api.example.com', { fetch: mock.fetch, accessToken: () => 'tok' }), calls: mock.calls };
}

const stored = { id: 'o1', bucket: 'avatars', name: 'u1/me.png', size: 3, mime_type: 'image/png', etag: '"e"' };

describe('files', () => {
  it('uploads raw bytes with POST, or PUT to replace', async () => {
    const { nelcota, calls } = client(json(stored, 201), json(stored, 200));
    const files = nelcota.storage.from('avatars');
    const blob = new Blob([new Uint8Array([1, 2, 3])], { type: 'image/png' });
    expect((await files.upload('u1/me.png', blob)).data).toEqual(stored);
    await files.upload('u1/me.png', new Uint8Array([1]), { upsert: true, contentType: 'image/png' });
    expect(calls.map((c) => [c.method, c.url.pathname, c.headers.get('content-type')])).toEqual([
      ['POST', '/storage/v1/object/avatars/u1/me.png', 'image/png'],
      ['PUT', '/storage/v1/object/avatars/u1/me.png', 'image/png'],
    ]);
    expect(calls[0]!.headers.get('authorization')).toBe('Bearer tok');
  });

  it('defaults the content type and refuses header injection', async () => {
    const { nelcota, calls } = client(json(stored, 201));
    await nelcota.storage.from('docs').upload('a.bin', new ArrayBuffer(2));
    expect(calls[0]!.headers.get('content-type')).toBe('application/octet-stream');
    await expect(
      nelcota.storage.from('docs').upload('a.txt', 'x', { contentType: 'text/plain\r\nx-evil: 1' }),
    ).rejects.toThrow(NelcotaUsageError);
  });

  it('encodes names and refuses traversal before sending', async () => {
    const { nelcota, calls } = client(empty(204));
    await nelcota.storage.from('docs').remove('reports/Q1 #2?.pdf');
    expect(calls[0]!.url.pathname).toBe('/storage/v1/object/docs/reports/Q1%20%232%3F.pdf');
    await expect(nelcota.storage.from('docs').remove('../other/x')).rejects.toThrow(NelcotaUsageError);
    expect(() => nelcota.storage.from('Docs')).toThrow(NelcotaUsageError);
    expect(calls).toHaveLength(1);
  });

  it('opens ranges and conditional downloads', async () => {
    const { nelcota, calls } = client(new Response('ab', { status: 206 }), new Response(null, { status: 304 }));
    const files = nelcota.storage.from('docs');
    const partial = await files.open('a.txt', { range: { start: 0, end: 1 } });
    expect(partial.data?.status).toBe(206);
    expect(await partial.data?.text()).toBe('ab');
    expect(calls[0]!.headers.get('range')).toBe('bytes=0-1');
    const unchanged = await files.open('a.txt', { ifNoneMatch: '"e"', download: true });
    expect(unchanged.data?.status).toBe(304);
    expect(calls[1]!.headers.get('if-none-match')).toBe('"e"');
    expect(calls[1]!.url.search).toBe('?download=');
    await expect(files.open('a.txt', { range: { start: 5, end: 1 } })).rejects.toThrow(NelcotaUsageError);
  });

  it('downloads a Blob', async () => {
    const { nelcota } = client(new Response('hello', { headers: { 'content-type': 'text/plain' } }));
    const { data } = await nelcota.storage.from('docs').download('a.txt');
    expect(await data?.text()).toBe('hello');
  });

  it('lists a folder', async () => {
    const { nelcota, calls } = client(json({ folders: ['b'], objects: [] }));
    const { data } = await nelcota.storage.from('docs').list({ prefix: 'a/', limit: 10 });
    expect(data).toEqual({ folders: ['b'], objects: [] });
    expect(calls[0]!.url.pathname).toBe('/storage/v1/object/list/docs');
    expect(JSON.parse(calls[0]!.body!)).toEqual({ prefix: 'a/', limit: 10 });
  });

  it('signed URLs become absolute, public URLs need no request', async () => {
    const { nelcota, calls } = client(json({ signed_url: '/storage/v1/object/sign/docs/a.txt?token=t' }));
    const files = nelcota.storage.from('docs');
    expect((await files.createSignedUrl('a.txt', 60)).data).toBe('https://api.example.com/storage/v1/object/sign/docs/a.txt?token=t');
    expect(JSON.parse(calls[0]!.body!)).toEqual({ expires_in: 60 });
    await expect(files.createSignedUrl('a.txt', 0)).rejects.toThrow(NelcotaUsageError);
    expect(files.publicUrl('img/logo 1.png')).toBe('https://api.example.com/storage/v1/object/public/docs/img/logo%201.png');
  });

  it('refuses a signed URL pointing elsewhere', async () => {
    const { nelcota } = client(json({ signed_url: 'https://evil.example.com/x' }));
    expect((await nelcota.storage.from('docs').createSignedUrl('a.txt', 60)).error?.code).toBe('invalid_response');
  });
});

describe('buckets', () => {
  it('creates, updates and deletes', async () => {
    const bucket = { id: 'avatars', public: true, file_size_limit: null, allowed_mime_types: ['image/*'], created_at: '', updated_at: '' };
    const { nelcota, calls } = client(json(bucket, 201), json(bucket), empty(204), json([bucket]));
    await nelcota.storage.createBucket('avatars', { public: true, allowed_mime_types: ['image/*'] });
    await nelcota.storage.updateBucket('avatars', { file_size_limit: 1024 });
    await nelcota.storage.deleteBucket('avatars');
    expect((await nelcota.storage.listBuckets()).data).toEqual([bucket]);
    expect(calls.map((c) => [c.method, c.url.pathname, c.body && JSON.parse(c.body)])).toEqual([
      ['POST', '/storage/v1/bucket', { id: 'avatars', public: true, allowed_mime_types: ['image/*'] }],
      ['PUT', '/storage/v1/bucket/avatars', { file_size_limit: 1024 }],
      ['DELETE', '/storage/v1/bucket/avatars', null],
      ['GET', '/storage/v1/bucket', null],
    ]);
  });
});
