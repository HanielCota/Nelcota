// The built package in a real Chromium, on another origin than the API, so
// CORS is exercised for real: preflights with Prefer, the exposed
// Content-Range, uploads, the session in localStorage, address-bar cleanup
// and the service_role refusal. Run by scripts/contract.mjs --browser.
//
//   node test/browser/smoke.mjs <api url> <service_role token>

import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { dirname, extname, join, normalize, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, firefox, webkit } from 'playwright-core';

const [api, serviceToken] = process.argv.slice(2);
if (!api || !serviceToken) {
  console.error('usage: smoke.mjs <api url> <service_role token>');
  process.exit(2);
}

const dist = resolve(dirname(fileURLToPath(import.meta.url)), '../../dist');
const examples = resolve(dirname(fileURLToPath(import.meta.url)), '../../examples');
const page = '<!doctype html><meta charset="utf-8"><title>sdk smoke</title><body>smoke</body>';

const server = createServer(async (request, response) => {
  const path = new URL(request.url ?? '/', 'http://x').pathname;
  if (path === '/' || path === '/app') {
    response.writeHead(200, { 'content-type': 'text/html' }).end(page);
    return;
  }
  if (path === '/demo' || path === '/examples/browser.js') {
    try {
      const file = path === '/demo' ? 'browser.html' : 'browser.js';
      response.writeHead(200, { 'content-type': file.endsWith('.html') ? 'text/html' : 'text/javascript' }).end(await readFile(join(examples, file)));
    } catch { response.writeHead(404).end(); }
    return;
  }
  const file = normalize(join(dist, path.replace(/^\/dist\//, '/')));
  if (!path.startsWith('/dist/') || !file.startsWith(dist)) {
    response.writeHead(404).end();
    return;
  }
  try {
    const type = extname(file) === '.js' ? 'text/javascript' : 'application/octet-stream';
    response.writeHead(200, { 'content-type': type }).end(await readFile(file));
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const origin = `http://127.0.0.1:${server.address().port}`;

const engine = process.env.NELCOTA_SDK_BROWSER ?? 'chromium';
if (!['chromium', 'firefox', 'webkit'].includes(engine)) throw new Error(`Unknown browser: ${engine}`);
const browser = await { chromium, firefox, webkit }[engine].launch(
  engine === 'chromium' && process.env.PLAYWRIGHT_CHROMIUM ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM } : {},
);
console.log(`Browser: ${engine}`);
let failed = false;
try {
  const context = await browser.newContext();
  const tab = await context.newPage();
  const failures = [];
  tab.on('console', (message) => {
    if (message.type() === 'error') failures.push(message.text());
  });
  await tab.goto(`${origin}/app#type=magiclink&token=abc123`);
  const result = await tab.evaluate(
    async ({ api, serviceToken }) => {
      const { createClient, NelcotaUsageError } = await import('/dist/index.js');
      const out = {};
      const nelcota = createClient(api);

      out.link = nelcota.auth.readEmailLink();
      out.cleanedUrl = location.href;

      const email = `browser-${crypto.randomUUID()}@example.com`;
      const signUp = await nelcota.auth.signUp({ email, password: 'browser-password-1' });
      out.signUpError = signUp.error?.code ?? null;
      out.stored = Object.keys(localStorage).filter((k) => k.startsWith('nelcota.'));

      const inserted = await nelcota.from('echo').insert({ value: 'from the browser' }).select('id,value').single();
      out.inserted = inserted.data?.value ?? inserted.error?.message;

      const counted = await nelcota.from('customers').select('id', { count: 'exact' }).limit(1);
      out.count = counted.count;

      const uid = signUp.data?.user.id;
      const files = nelcota.storage.from('private-files');
      const up = await files.upload(`${uid}/browser.txt`, new Blob(['blob body'], { type: 'text/plain' }));
      out.upload = up.error?.code ?? up.data?.size;
      const opened = await files.open(`${uid}/browser.txt`);
      out.etagReadable = Boolean(opened.data?.headers.get('etag'));
      out.download = await opened.data?.text();

      const admin = createClient(api, { accessToken: () => serviceToken });
      try {
        await admin.from('customers').select();
        out.serviceRole = 'allowed';
      } catch (error) {
        out.serviceRole = error instanceof NelcotaUsageError ? 'refused' : String(error);
      }

      const signOut = await nelcota.auth.signOut();
      out.signOut = signOut.error?.code ?? 'ok';
      out.storedAfter = Object.keys(localStorage).filter((k) => k.startsWith('nelcota.'));
      return out;
    },
    { api, serviceToken },
  );

  const expected = {
    link: { type: 'magiclink', token: 'abc123' },
    cleanedUrl: `${origin}/app`,
    signUpError: null,
    stored: [`nelcota.${new URL(api).host}.session`],
    inserted: 'from the browser',
    count: 3,
    upload: 9,
    etagReadable: true,
    download: 'blob body',
    serviceRole: 'refused',
    signOut: 'ok',
    storedAfter: [],
  };
  for (const [key, value] of Object.entries(expected)) {
    const ok = JSON.stringify(result[key]) === JSON.stringify(value);
    failed ||= !ok;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${key}: ${JSON.stringify(result[key])}${ok ? '' : ` (expected ${JSON.stringify(value)})`}`);
  }
  if (failures.length > 0) console.log(`console errors:\n  ${failures.join('\n  ')}`);

  // Two actual pages share localStorage and Web Locks, rather than a mocked
  // lock manager. Rotation must leave both pages with the same live session.
  await tab.evaluate(async (api) => {
    const { createClient } = await import('/dist/index.js');
    globalThis.sdkClient = createClient(api, { auth: { autoRefresh: false } });
    const signup = await globalThis.sdkClient.auth.signUp({ email: `tabs-${crypto.randomUUID()}@example.com`, password: 'browser-tabs-password' });
    if (signup.error) throw signup.error;
    const session = (await globalThis.sdkClient.auth.getSession()).data;
    await globalThis.sdkClient.auth.setSession({ ...session, expires_at: Math.floor(Date.now() / 1000) + 5 });
  }, api);
  const other = await context.newPage();
  await other.goto(`${origin}/app`);
  await other.evaluate(async (api) => {
    const { createClient } = await import('/dist/index.js');
    globalThis.sdkClient = createClient(api, { auth: { autoRefresh: false } });
  }, api);
  const sessions = await Promise.all([tab, other].map((page) => page.evaluate(() => globalThis.sdkClient.auth.getSession())));
  const shared = sessions.every((value) => value.error === null && value.data !== null)
    && sessions[0].data.refresh_token === sessions[1].data.refresh_token;
  failed ||= !shared;
  console.log(`${shared ? 'ok  ' : 'FAIL'} refresh across two real tabs`);
  await tab.evaluate(() => globalThis.sdkClient.auth.signOut());
  await other.waitForFunction(async () => (await globalThis.sdkClient.auth.getSession()).data === null);
  console.log('ok  sign-out broadcast to the other tab');
  await Promise.all([tab, other].map((page) => page.evaluate(() => globalThis.sdkClient.dispose())));

  await other.goto(`${origin}/demo`);
  await other.getByLabel('API URL').fill(api);
  await other.getByLabel('Email', { exact: true }).fill(`demo-${Date.now()}-${engine}@example.com`);
  await other.getByLabel('Password', { exact: true }).fill('browser-demo-password');
  await other.getByRole('button', { name: 'Create account', exact: true }).click();
  await other.waitForFunction(() => document.querySelector('#result').textContent.includes('Account created'));
  await other.getByLabel('Note', { exact: true }).fill('Hello from the browser demo');
  await other.getByRole('button', { name: 'Save note', exact: true }).click();
  await other.waitForFunction(() => document.querySelector('#result').textContent.includes('Hello from the browser demo'));
  await other.getByLabel('File', { exact: true }).setInputFiles({ name: 'demo.txt', mimeType: 'text/plain', buffer: Buffer.from('demo contents') });
  await other.getByRole('button', { name: 'Upload file', exact: true }).click();
  await other.waitForFunction(() => document.querySelector('#result').textContent.includes('demo.txt'));
  await other.getByRole('button', { name: 'List notes', exact: true }).click();
  await other.waitForFunction(() => document.querySelector('#result').textContent.includes('Hello from the browser demo'));
  await other.getByRole('button', { name: 'Sign out', exact: true }).click();
  await other.waitForFunction(() => document.querySelector('#result').textContent === 'You are signed out.');
  console.log('ok  runnable browser demo: signup, notes, upload and logout');
} finally {
  await browser.close();
  server.close();
}
process.exit(failed ? 1 : 0);
