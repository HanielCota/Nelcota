import fc from 'fast-check';
import { describe, expect, it } from 'vitest';
import { escapeLike } from '../../src/index.js';
import { signedIn, skip, visitor } from './setup.js';

describe.skipIf(skip)('REST against the server', () => {
  it('reads as a visitor with order, paging and an exact count', async () => {
    const nelcota = visitor();
    const { data, error, count } = await nelcota
      .from('customers')
      .select('id,name', { count: 'exact' })
      .order('id', { ascending: false })
      .range(0, 1);
    expect(error).toBeNull();
    expect(count).toBe(3);
    expect(data?.map((c) => c.name)).toEqual(['Ruler, "30cm" (wood)', 'Bruno']);

    const head = await nelcota.from('customers').select('*', { count: 'exact', head: true });
    expect(head).toMatchObject({ data: null, error: null, count: 3 });
  });

  it('reports the returned range, offsets and walks every page', async () => {
    const nelcota = visitor();
    const second = await nelcota.from('customers').select('id').order('id').limit(1).offset(1);
    expect(second).toMatchObject({ error: null, range: { from: 1, to: 1 }, count: null });
    const empty = await nelcota.from('customers').select('id').order('id').offset(100);
    expect(empty).toMatchObject({ data: [], error: null, range: null });

    const all = (await nelcota.from('customers').select('id').order('id').throwOnError()).data;
    const paged: unknown[] = [];
    for await (const page of nelcota.from('customers').select('id').order('id').pages(2)) paged.push(...page);
    expect(paged).toEqual(all);
  });

  it('finds values with delimiters through every operator form', async () => {
    const nelcota = visitor();
    const name = 'Ruler, "30cm" (wood)';
    for (const query of [
      nelcota.from('customers').select('name').eq('name', name),
      nelcota.from('customers').select('name').in('name', [name, 'nobody']),
      nelcota.from('customers').select('name').or((c) => [c.eq('name', name), c.eq('name', 'x,y)')]),
      nelcota.from('customers').select('name').ilike('name', `*${escapeLike('"30cm" (')}*`),
    ]) {
      const { data, error } = await query;
      expect(error).toBeNull();
      expect(data).toEqual([{ name }]);
    }
    const single = await nelcota.from('customers').select('name').eq('name', 'Ana').single();
    expect(single.data).toEqual({ name: 'Ana' });
    const many = await nelcota.from('customers').select('name').single();
    expect(many.error?.code).toBe('not_single');
  });

  it('round-trips arbitrary text through eq, in and or groups', async () => {
    const { client } = await signedIn();
    const specials = fc.constantFrom(',', '(', ')', '"', '\\', '.', '*', '%', '_', ' ', '&', '=', '#', '?', '+', ':', '!', "'", ';', '\n', '\t');
    const text = fc
      .array(fc.oneof(specials, fc.string({ unit: 'grapheme', minLength: 1, maxLength: 1 })), { minLength: 1, maxLength: 12 })
      .map((parts) => parts.join(''))
      .filter((s) => !s.includes('\u0000'));
    await fc.assert(
      fc.asyncProperty(text, async (value) => {
        const inserted = await client.from('echo').insert({ value }).select('id,value').single();
        expect(inserted.error).toBeNull();
        expect(inserted.data?.value).toBe(value);
        const id = inserted.data!.id;
        const queries = [
          client.from('echo').select('id').eq('value', value),
          client.from('echo').select('id').in('value', [value]),
          client.from('echo').select('id').or((c) => [c.eq('value', value), c.and([c.eq('value', value), c.is('slug', null)])]),
        ];
        for (const query of queries) {
          const { data, error } = await query;
          expect(error).toBeNull();
          expect(data).toEqual([{ id }]);
        }
        await client.from('echo').delete().eq('id', id);
      }),
      { numRuns: 40 },
    );
  });

  it('writes under RLS: each user sees only their own rows', async () => {
    const ana = await signedIn();
    const bruno = await signedIn();
    const created = await ana.client.from('orders').insert({ total: 30, customer_id: 1 }).select('id,owner,total,status').single();
    expect(created.error).toBeNull();
    expect(created.data).toMatchObject({ owner: ana.id, total: 30, status: 'open' });
    const id = created.data!.id;

    expect((await bruno.client.from('orders').select('id').eq('id', id)).data).toEqual([]);
    const theft = await bruno.client.from('orders').update({ status: 'paid' }).eq('id', id).select('id');
    expect(theft.data).toEqual([]);

    const updated = await ana.client.from('orders').update({ status: 'paid' }).eq('id', id).select('status').single();
    expect(updated.data).toEqual({ status: 'paid' });
    const removed = await ana.client.from('orders').delete().eq('id', id).select('id');
    expect(removed.data).toEqual([{ id }]);
  });

  it('embeds related rows with their own filters, order and limit', async () => {
    const { client } = await signedIn();
    const order = await client.from('orders').insert({ customer_id: 2, total: 9 }).select('id').single();
    const orderId = order.data!.id;
    await client.from('items').insert([
      { order_id: orderId, product: 'Pen', qty: 2 },
      { order_id: orderId, product: 'Ruler', qty: 1 },
      { order_id: orderId, product: 'Ink', qty: 5 },
    ]);
    const { data, error } = await client
      .from('orders')
      .select('id,customer:customers(name),items(product,qty)')
      .eq('id', orderId)
      .gte('items.qty', 2)
      .order('qty', { referencedTable: 'items', ascending: false })
      .limit(1, { referencedTable: 'items' })
      .single();
    expect(error).toBeNull();
    expect(data).toEqual({ id: orderId, customer: { name: 'Bruno' }, items: [{ product: 'Ink', qty: 5 }] });
  });

  it('upserts by a unique column', async () => {
    const { client } = await signedIn();
    const slug = `slug-${Date.now()}`;
    await client.from('echo').upsert({ value: 'first', slug }, { onConflict: 'slug' });
    await client.from('echo').upsert({ value: 'second', slug }, { onConflict: 'slug' });
    await client.from('echo').upsert({ value: 'ignored', slug }, { onConflict: 'slug', ignoreDuplicates: true });
    expect((await client.from('echo').select('value').eq('slug', slug)).data).toEqual([{ value: 'second' }]);
  });

  it('calls functions with the caller role', async () => {
    const anon = visitor();
    expect((await anon.rpc('add', { a: 2 })).data).toBe(12);
    expect((await anon.rpc('whoami')).data).toBe('anon');
    expect(await anon.rpc('nothing')).toEqual({ data: null, error: null, status: 204 });
    const ana = await signedIn();
    expect((await ana.client.rpc('whoami')).data).toBe(ana.id);
  });

  it('passes server errors through untouched', async () => {
    const anon = visitor();
    const denied = await anon.from('orders').select();
    expect(denied.error).toMatchObject({ status: 401, code: 'db_error' });
    const { client } = await signedIn();
    const noFilter = await client.from('orders').delete();
    expect(noFilter.error).toMatchObject({ status: 400, code: 'invalid_query' });
    const badValue = await client.from('orders').select().eq('total', 'abc');
    expect(badValue.error?.status).toBe(400);
  });
});
