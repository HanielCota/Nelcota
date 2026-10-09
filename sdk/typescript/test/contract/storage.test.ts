import { describe, expect, it } from 'vitest';
import { NelcotaUsageError } from '../../src/index.js';
import { serviceClient, signedIn, skip, visitor } from './setup.js';

describe.skipIf(skip)('storage against the server', () => {
  it('uploads, downloads, ranges, ETags, lists, replaces and removes', async () => {
    const { client, id } = await signedIn();
    const files = client.storage.from('private-files');
    const name = `${id}/notes/hello world.txt`;

    const uploaded = await files.upload(name, new Blob(['hello, nelcota'], { type: 'text/plain' }));
    expect(uploaded.error).toBeNull();
    expect(uploaded.data).toMatchObject({ bucket: 'private-files', name, size: 14 });
    expect((await files.upload(name, 'again')).error?.code).toBe('object_exists');

    expect(await (await files.download(name)).data?.text()).toBe('hello, nelcota');
    const part = await files.open(name, { range: { start: 7, end: 13 } });
    expect(part.data?.status).toBe(206);
    expect(await part.data?.text()).toBe('nelcota');
    const etag = (await files.open(name)).data!.headers.get('etag')!;
    expect((await files.open(name, { ifNoneMatch: etag })).data?.status).toBe(304);

    const listing = await files.list({ prefix: `${id}/` });
    expect(listing.data?.folders).toEqual(['notes']);
    expect((await files.list({ prefix: `${id}/notes/` })).data?.objects.map((o) => o.name)).toEqual([name]);

    const replaced = await files.upload(name, 'replaced', { upsert: true });
    expect(replaced.error).toBeNull();
    expect(await (await files.download(name)).data?.text()).toBe('replaced');

    expect((await files.remove(name)).error).toBeNull();
    expect((await files.download(name)).error?.status).toBe(404);
  });

  it('streams an upload without buffering it in the client', async () => {
    const { client, id } = await signedIn();
    const chunks = ['a'.repeat(1000), 'b'.repeat(1000), 'c'];
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        for (const chunk of chunks) controller.enqueue(new TextEncoder().encode(chunk));
        controller.close();
      },
    });
    const files = client.storage.from('private-files');
    const { data, error } = await files.upload(`${id}/stream.txt`, stream, { contentType: 'text/plain' });
    expect(error).toBeNull();
    expect(data?.size).toBe(2001);
  });

  it('policies keep users out of each other\'s folders', async () => {
    const ana = await signedIn();
    const bruno = await signedIn();
    const name = `${ana.id}/secret.txt`;
    await ana.client.storage.from('private-files').upload(name, 'secret');
    const intrusion = await bruno.client.storage.from('private-files').upload(`${ana.id}/x.txt`, 'x');
    expect(intrusion.error?.status).toBeGreaterThanOrEqual(400);
    expect((await bruno.client.storage.from('private-files').download(name)).error?.status).toBe(404);
    expect((await visitor().storage.from('private-files').download(name)).error).not.toBeNull();
  });

  it('signed URLs work without a session; public URLs for public buckets', async () => {
    const { client, id } = await signedIn();
    const privateFiles = client.storage.from('private-files');
    await privateFiles.upload(`${id}/shared.txt`, 'shared text');
    const signed = await privateFiles.createSignedUrl(`${id}/shared.txt`, 60);
    expect(signed.error).toBeNull();
    expect(await (await fetch(signed.data!)).text()).toBe('shared text');

    const publicFiles = client.storage.from('public-files');
    await publicFiles.upload(`${id}/logo 1.txt`, 'public text');
    expect(await (await fetch(publicFiles.publicUrl(`${id}/logo 1.txt`))).text()).toBe('public text');
  });

  it('refuses traversal before any request', async () => {
    const { client } = await signedIn();
    await expect(client.storage.from('private-files').download('../etc/passwd')).rejects.toThrow(NelcotaUsageError);
  });

  it('manages buckets with service_role', async () => {
    const admin = serviceClient();
    const id = `sdk-${Date.now()}`;
    const created = await admin.storage.createBucket(id, { public: false, allowed_mime_types: ['text/plain'] });
    expect(created.error).toBeNull();
    expect(created.data).toMatchObject({ id, public: false, allowed_mime_types: ['text/plain'] });
    const updated = await admin.storage.updateBucket(id, { public: true, allowed_mime_types: ['text/plain'] });
    expect(updated.error).toBeNull();
    expect((await admin.storage.getBucket(id)).data?.public).toBe(true);
    expect((await admin.storage.listBuckets()).data?.some((b) => b.id === id)).toBe(true);
    expect((await admin.storage.deleteBucket(id)).error).toBeNull();
    expect((await visitor().storage.createBucket(`${id}-x`)).error?.status).toBeGreaterThanOrEqual(400);
  });
});
