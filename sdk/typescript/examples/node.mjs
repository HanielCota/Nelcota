// NELCOTA_URL, NELCOTA_EMAIL and NELCOTA_PASSWORD; optional NELCOTA_SIGN_UP=1.
import { createClient } from '@nelcota/client';

export async function runExample({ url, email, password, signUp = false }) {
  if (!url || !email || !password) throw new Error('Set NELCOTA_URL, NELCOTA_EMAIL and NELCOTA_PASSWORD');
  const client = createClient(url);
  const take = (result) => { if (result.error) throw result.error; return result.data; };
  try {
    if (signUp) {
      const account = take(await client.auth.signUp({ email, password }));
      if (!account.session) throw new Error('Confirm the signup email, then run again with NELCOTA_SIGN_UP unset');
    } else take(await client.auth.signInWithPassword({ email, password }));
    const session = take(await client.auth.getSession());
    const note = take(await client.from('notes').insert({ body: 'Hello from Node' }).select('id,body').single());
    const notes = take(await client.from('notes').select('id,body').order('id', { ascending: false }).limit(20));
    const files = client.storage.from('files');
    const path = `${session.user.id}/node-example.txt`;
    take(await files.upload(path, 'Hello from Node', { upsert: true }));
    const downloaded = take(await files.download(path));
    const text = await downloaded.text();
    take(await files.remove(path));
    take(await client.from('notes').delete().eq('id', note.id));
    take(await client.auth.signOut());
    console.log({ note, notes, downloaded: text });
    return { note, notes, downloaded: text };
  } finally { client.dispose(); }
}

// Importing this function from a test does not start a demo account or request.
if (process.argv[1] && import.meta.url === (await import('node:url')).pathToFileURL(process.argv[1]).href) {
  await runExample({ url: process.env.NELCOTA_URL, email: process.env.NELCOTA_EMAIL,
    password: process.env.NELCOTA_PASSWORD, signUp: process.env.NELCOTA_SIGN_UP === '1' });
}
