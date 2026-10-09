import { describe, expectTypeOf, it } from 'vitest';
import { createClient, type SelectError } from '../../src/index.js';
import type { Database, Json } from './database.js';

const nelcota = createClient<Database>('https://api.example.com');

type Result<Q> = Q extends PromiseLike<infer R> ? (R extends { data: infer D; error: null } ? D : never) : never;

describe('select', () => {
  it('types * as the full row', () => {
    const all = nelcota.from('orders').select();
    expectTypeOf<Result<typeof all>>().toEqualTypeOf<Database['public']['Tables']['orders']['Row'][]>();
    const star = nelcota.from('orders').select('*');
    expectTypeOf<Result<typeof star>>().toEqualTypeOf<Database['public']['Tables']['orders']['Row'][]>();
  });

  it('picks columns', () => {
    const query = nelcota.from('orders').select('id, status');
    expectTypeOf<Result<typeof query>>().toEqualTypeOf<{ id: number; status: 'open' | 'paid' | 'shipped' }[]>();
  });

  it('types many-to-one embeds as an object or null, one-to-many as arrays', () => {
    const query = nelcota.from('orders').select('id,buyer:customers!customer_id(name),items(product,qty)');
    expectTypeOf<Result<typeof query>>().toEqualTypeOf<
      { id: number; buyer: { name: string } | null; items: { product: string; qty: number }[] }[]
    >();
    const back = nelcota.from('customers').select('name,orders!customer_id(id,items(*))');
    expectTypeOf<Result<typeof back>>().toEqualTypeOf<
      { name: string; orders: { id: number; items: Database['public']['Tables']['items']['Row'][] }[] }[]
    >();
  });

  it('reports ambiguous embeds and unknown columns in the row type', () => {
    const ambiguous = nelcota.from('orders').select('customers(name)');
    expectTypeOf<Result<typeof ambiguous>[number]['customers']>().toMatchTypeOf<SelectError<string>>();
    const unknown = nelcota.from('orders').select('nope');
    expectTypeOf<Result<typeof unknown>[number]['nope']>().toMatchTypeOf<SelectError<string>>();
  });

  it('single and maybeSingle change the result to one row', () => {
    const one = nelcota.from('orders').select('id').eq('id', 1).single();
    expectTypeOf<Result<typeof one>>().toEqualTypeOf<{ id: number }>();
    const maybe = nelcota.from('orders').select('id').maybeSingle();
    expectTypeOf<Result<typeof maybe>>().toEqualTypeOf<{ id: number } | null>();
  });
});

describe('filters', () => {
  it('checks columns and value types', () => {
    const orders = nelcota.from('orders').select();
    orders.eq('status', 'paid');
    orders.in('status', ['open', 'paid']);
    orders.gte('total', 10);
    orders.eq('items.qty', 3);
    // @ts-expect-error not a status
    orders.eq('status', 'lost');
    // @ts-expect-error not a column
    orders.eq('nope', 1);
    // @ts-expect-error total is a number
    orders.gt('total', true);
    orders.or((c) => [c.eq('status', 'paid'), c.gt('total', 100)]);
    // Columns that are not selected can still filter and order.
    nelcota.from('orders').select('id').eq('status', 'paid').order('total').or((c) => [c.gt('total', 1)]);
    // @ts-expect-error still a column of the table
    nelcota.from('orders').select('id').eq('nope', 1);
    // @ts-expect-error not a column
    orders.or((c) => [c.eq('nope', 1)]);
    // @ts-expect-error not a status
    orders.or((c) => [c.eq('status', 'lost')]);
  });
});

describe('writes', () => {
  it('takes Insert and Update shapes', () => {
    nelcota.from('orders').insert({ total: 10 });
    nelcota.from('orders').insert([{ total: 10, status: 'paid' }]);
    // @ts-expect-error total is required
    nelcota.from('orders').insert({ status: 'paid' });
    nelcota.from('orders').update({ status: 'shipped' }).eq('id', 1);
    // @ts-expect-error unknown column
    nelcota.from('orders').update({ nope: 1 });
    const meta: Json = { a: 1 };
    nelcota.from('orders').upsert({ total: 1, meta }, { onConflict: 'id' });
  });

  it('returns nothing unless select() asks for rows', () => {
    const minimal = nelcota.from('orders').insert({ total: 1 });
    expectTypeOf<Result<typeof minimal>>().toEqualTypeOf<null>();
    const back = nelcota.from('orders').insert({ total: 1 }).select('id').single();
    expectTypeOf<Result<typeof back>>().toEqualTypeOf<{ id: number }>();
  });

  it('views without Insert refuse writes', () => {
    // @ts-expect-error a materialized view has no Insert
    nelcota.from('order_totals').insert({ total: 1 });
  });
});

describe('rpc', () => {
  it('types arguments and results', async () => {
    expectTypeOf((await nelcota.rpc('add', { a: 1 })).data).toEqualTypeOf<number | null>();
    // @ts-expect-error a is required
    void nelcota.rpc('add', {});
    expectTypeOf((await nelcota.rpc('open_orders')).data).toEqualTypeOf<Database['public']['Tables']['orders']['Row'][] | null>();
    // @ts-expect-error unknown function
    void nelcota.rpc('nope');
  });
});

describe('untyped client', () => {
  it('works without a Database type', () => {
    const loose = createClient('https://api.example.com');
    const query = loose.from('anything').select('a,b(c)').eq('x', 1);
    expectTypeOf<Result<typeof query>>().toEqualTypeOf<Record<string, any>[]>();
  });
});
