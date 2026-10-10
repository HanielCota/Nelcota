import { createClient } from '@nelcota/client';

let client;
const output = document.querySelector('#result');
const take = (result) => { if (result.error) throw result.error; return result.data; };
const show = (value) => { output.textContent = typeof value === 'string' ? value : JSON.stringify(value, null, 2); };
async function action(task) {
  try { show(await task()); } catch (error) { show(`${error.code ?? error.name}: ${error.message}`); }
}
function signedIn() { if (!client) throw new Error('Entre primeiro.'); return client; }

document.querySelector('#auth').addEventListener('submit', (event) => {
  event.preventDefault();
  const values = new FormData(event.currentTarget);
  const signup = event.submitter?.value === 'signup';
  void action(async () => {
    client?.dispose();
    client = createClient(String(values.get('url')));
    const credentials = { email: String(values.get('email')), password: String(values.get('password')) };
    if (signup) {
      const account = take(await client.auth.signUp(credentials));
      return account.session ? 'Conta criada. Você entrou.' : 'Confirme seu email e depois entre.';
    }
    take(await client.auth.signInWithPassword(credentials));
    return 'Você entrou.';
  });
});
document.querySelector('#note').addEventListener('submit', (event) => {
  event.preventDefault();
  const values = new FormData(event.currentTarget);
  void action(async () => take(await signedIn().from('notes').insert({ body: String(values.get('body')) }).select('id,body').single()));
});
document.querySelector('#file').addEventListener('submit', (event) => {
  event.preventDefault();
  const file = new FormData(event.currentTarget).get('file');
  void action(async () => {
    const sdk = signedIn();
    const session = take(await sdk.auth.getSession());
    if (!session) throw new Error('Entre primeiro.');
    return take(await sdk.storage.from('files').upload(`${session.user.id}/${file.name}`, file, { upsert: true }));
  });
});
document.querySelector('#list').addEventListener('click', () => void action(async () =>
  take(await signedIn().from('notes').select('id,body').order('id', { ascending: false }).limit(20))));
document.querySelector('#logout').addEventListener('click', () => void action(async () => {
  take(await signedIn().auth.signOut());
  return 'Você saiu.';
}));
window.addEventListener('pagehide', () => client?.dispose());
