// Compiled (not run) by scripts/contract.mjs against the types the real
// `nelcota types` generated for test/contract/fixture.sql: the SDK's typing
// must accept the generator's output as is.
import { createClient } from '../../src/index.js';
import type { Database } from './generated/database.js';

const nelcota = createClient<Database>('http://127.0.0.1');

export async function typed(): Promise<void> {
  const { data } = await nelcota
    .from('orders')
    .select('id,status,customer:customers(name),items(product,qty)')
    .eq('status', 'paid')
    .single();
  if (data) {
    const status: 'open' | 'paid' | 'shipped' = data.status;
    const name: string | undefined = data.customer?.name;
    const qty: number | undefined = data.items[0]?.qty;
    void [status, name, qty];
  }
  // @ts-expect-error not a status of the enum
  void nelcota.from('orders').select().eq('status', 'lost');
  const sum: number | null = (await nelcota.rpc('add', { a: 1 })).data;
  void sum;
}
