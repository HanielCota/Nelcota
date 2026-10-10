// The todos example (examples/todos.sql) through @nelcota/client, from Node.
//
//   psql "$NELCOTA_DATABASE_URL" -f examples/todos.sql
//   npm install @nelcota/client
//   NELCOTA_URL=http://127.0.0.1:8000 node examples/sdk-todos.mjs

import { createClient } from '@nelcota/client';

const nelcota = createClient(process.env.NELCOTA_URL ?? 'http://127.0.0.1:8000');

const email = `demo-${Date.now()}@example.com`;
const signUp = await nelcota.auth.signUp({ email, password: 'a-demo-password' });
if (signUp.error) throw signUp.error;
if (!signUp.data.session) throw new Error('Confirm the signup email before using authenticated CRUD');

// RLS fills user_id from the token (DEFAULT auth.uid()) and hides other users' rows.
const { data: created, error } = await nelcota
  .from('todos')
  .insert([{ title: 'Write the migration' }, { title: 'Ship it' }])
  .select('id,title,done');
if (error) throw error;
console.log('created', created);

await nelcota.from('todos').update({ done: true }).eq('id', created[0].id);

const { data: open, count } = await nelcota
  .from('todos')
  .select('id,title', { count: 'exact' })
  .eq('done', false)
  .order('id');
console.log(`${count} open`, open);

await nelcota.auth.signOut();
