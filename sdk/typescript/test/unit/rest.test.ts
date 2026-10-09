import { describe, expect, it } from 'vitest';
import { createClient, escapeLike, NelcotaUsageError } from '../../src/index.js';
import { countFromRange } from '../../src/rest/query.js';
import { empty, json, mockFetch } from './helpers.js';

function client(...replies: Parameters<typeof mockFetch>) {
  const mock = mockFetch(...replies);
  const nelcota = createClient('https://api.example.com/', { fetch: mock.fetch, auth: { autoRefresh: false } });
  return { nelcota, calls: mock.calls };
}

const query = (url: URL) => [...url.searchParams.entries()];

describe('reads', () => {
  it('sends one GET with the filters, order and paging', async () => {
    const { nelcota, calls } = client(json([{ id: 1 }], 200, { 'content-range': '0-0/1' }));
    const { data, error, count, status } = await nelcota
      .from('todos')
      .select('id, title, owner:users(email)', { count: 'exact' })
      .eq('done', false)
      .in('priority', ['high', 'a,"b"'])
      .ilike('title', `*${escapeLike('50%')}*`)
      .is('deleted_at', null)
      .not('status', 'eq', 'cancelled')
      .gte('orders.total', 5)
      .order('id', { ascending: false, nullsFirst: false })
      .order('title')
      .order('qty', { referencedTable: 'orders' })
      .range(20, 39)
      .limit(2, { referencedTable: 'orders' });
    expect(error).toBeNull();
    expect(data).toEqual([{ id: 1 }]);
    expect(count).toBe(1);
    expect(status).toBe(200);
    expect(calls).toHaveLength(1);
    const [call] = calls;
    expect(call!.method).toBe('GET');
    expect(call!.url.pathname).toBe('/rest/v1/todos');
    expect(call!.headers.get('prefer')).toBe('count=exact');
    expect(query(call!.url)).toEqual([
      ['select', 'id,title,owner:users(email)'],
      ['done', 'eq.false'],
      ['priority', 'in.("high","a,\\"b\\"")'],
      ['title', 'ilike.*50\\%*'],
      ['deleted_at', 'is.null'],
      ['status', 'not.eq.cancelled'],
      ['orders.total', 'gte.5'],
      ['order', 'id.desc.nullslast,title.asc'],
      ['orders.order', 'qty.asc'],
      ['offset', '20'],
      ['limit', '20'],
      ['orders.limit', '2'],
    ]);
  });

  it('builds or/and groups from typed conditions', async () => {
    const { nelcota, calls } = client(json([]));
    await nelcota
      .from('products')
      .select()
      .or((c) => [c.eq('name', 'Ruler, 30cm'), c.and([c.gt('price', 100), c.not(c.is('featured', true))])])
      .or((c) => [c.eq('qty', 1)], { referencedTable: 'items' });
    expect(query(calls[0]!.url)).toEqual([
      ['select', '*'],
      ['or', '(name.eq."Ruler, 30cm",and(price.gt."100",featured.not.is.true))'],
      ['items.or', '(qty.eq."1")'],
    ]);
  });

  it('keeps builders immutable, so a base query can be reused', async () => {
    const { nelcota, calls } = client(json([]));
    const base = nelcota.from('todos').select('id').eq('done', false);
    await base.eq('id', 1);
    await base.eq('id', 2);
    expect(query(calls[0]!.url)).toEqual([['select', 'id'], ['done', 'eq.false'], ['id', 'eq.1']]);
    expect(query(calls[1]!.url)).toEqual([['select', 'id'], ['done', 'eq.false'], ['id', 'eq.2']]);
  });

  it('single asks for two rows at most and checks there is exactly one', async () => {
    const one = client(json([{ id: 1 }]));
    expect((await one.nelcota.from('todos').select().eq('id', 1).single()).data).toEqual({ id: 1 });
    expect(one.calls[0]!.url.searchParams.get('limit')).toBe('2');

    const many = client(json([{ id: 1 }, { id: 2 }]));
    const result = await many.nelcota.from('todos').select().single();
    expect(result.error?.code).toBe('not_single');
    expect(result.error?.message).toContain('more than one');

    const none = client(json([]));
    expect((await none.nelcota.from('todos').select().maybeSingle()).data).toBeNull();
    expect((await none.nelcota.from('todos').select().single()).error?.code).toBe('not_single');
  });

  it('head counts without rows', async () => {
    const { nelcota, calls } = client(empty(200, { 'content-range': '*/137' }));
    const { data, count } = await nelcota.from('todos').select('*', { count: 'exact', head: true });
    expect(calls[0]!.method).toBe('HEAD');
    expect(data).toBeNull();
    expect(count).toBe(137);
  });

  it('reads totals from Content-Range', () => {
    expect(countFromRange('0-19/137')).toBe(137);
    expect(countFromRange('0-19/*')).toBeNull();
    expect(countFromRange('*/0')).toBe(0);
    expect(countFromRange(null)).toBeNull();
  });

  it('returns server errors as values', async () => {
    const { nelcota } = client(json({ code: 'db_error', message: 'permission denied for table secrets (42501)' }, 401));
    const { data, error, status } = await nelcota.from('secrets').select();
    expect(data).toBeNull();
    expect(status).toBe(401);
    expect(error).toMatchObject({ name: 'NelcotaError', status: 401, code: 'db_error' });
  });

  it('refuses unsafe identifiers before sending', () => {
    const { nelcota, calls } = client(json([]));
    expect(() => nelcota.from('pg_catalog.pg_authid')).toThrow(NelcotaUsageError);
    expect(() => nelcota.from('todos').select().eq('id;drop' as 'id', 1)).not.toThrow();
    expect(() => nelcota.from('todos').select().eq('a,b' as 'id', 1)).toThrow(NelcotaUsageError);
    expect(() => nelcota.from('todos').select('id"')).toThrow(NelcotaUsageError);
    expect(() => nelcota.from('todos').select().order('id.desc' as 'id')).toThrow(NelcotaUsageError);
    expect(() => nelcota.from('todos').select().limit(-1)).toThrow(NelcotaUsageError);
    expect(() => nelcota.from('todos').select().range(5, 4)).toThrow(NelcotaUsageError);
    expect(calls).toHaveLength(0);
  });
});

describe('writes', () => {
  it('insert sends rows with return=minimal, select() asks for them back', async () => {
    const { nelcota, calls } = client(empty(201), json([{ id: 1, title: 'a' }], 201));
    expect((await nelcota.from('todos').insert([{ title: 'a' }, { title: 'b' }])).data).toBeNull();
    expect(calls[0]!.headers.get('prefer')).toBe('return=minimal');
    expect(calls[0]!.body).toBe('[{"title":"a"},{"title":"b"}]');

    const { data } = await nelcota.from('todos').insert({ title: 'a' }).select('id,title').single();
    expect(data).toEqual({ id: 1, title: 'a' });
    expect(calls[1]!.headers.get('prefer')).toBe('return=representation');
    expect(query(calls[1]!.url)).toEqual([['select', 'id,title']]);
  });

  it('upsert names the conflict columns and the resolution', async () => {
    const { nelcota, calls } = client(empty(201));
    await nelcota.from('products').upsert({ slug: 'pen' }, { onConflict: ['slug', 'shop'], ignoreDuplicates: true });
    expect(query(calls[0]!.url)).toEqual([['on_conflict', 'slug,shop']]);
    expect(calls[0]!.headers.get('prefer')).toBe('return=minimal,resolution=ignore-duplicates');
  });

  it('update and delete carry their filters', async () => {
    const { nelcota, calls } = client(empty(204));
    await nelcota.from('todos').update({ done: true }).eq('id', 3);
    await nelcota.from('todos').delete().in('id', [1, 2]);
    expect(calls.map((c) => [c.method, c.url.search])).toEqual([
      ['PATCH', '?id=eq.3'],
      ['DELETE', '?id=in.%28%221%22%2C%222%22%29'],
    ]);
    expect(calls[0]!.body).toBe('{"done":true}');
  });

  it('refuses malformed writes before sending', () => {
    const { nelcota } = client(empty());
    expect(() => nelcota.from('todos').insert([])).toThrow(NelcotaUsageError);
    expect(() => nelcota.from('todos').update([] as never)).toThrow(NelcotaUsageError);
    expect(() => nelcota.from('todos').upsert({}, { onConflict: 'a b' })).toThrow(NelcotaUsageError);
  });
});

describe('rpc', () => {
  it('posts named arguments and reads the result', async () => {
    const { nelcota, calls } = client(json(12), empty(204));
    expect(await nelcota.rpc('add', { a: 2 })).toEqual({ data: 12, error: null, status: 200 });
    expect(calls[0]!.url.pathname).toBe('/rest/v1/rpc/add');
    expect(calls[0]!.body).toBe('{"a":2}');
    expect((await nelcota.rpc('nothing')).data).toBeNull();
    expect(calls[1]!.body).toBe('{}');
  });
});
