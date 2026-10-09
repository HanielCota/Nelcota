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
import { chromium } from 'playwright-core';

const [api, serviceToken] = process.argv.slice(2);
if (!api || !serviceToken) {
  console.error('usage: smoke.mjs <api url> <service_role token>');
  process.exit(2);
}

const dist = resolve(dirname(fileURLToPath(import.meta.url)), '../../dist');
const page = '<!doctype html><meta charset="utf-8"><title>sdk smoke</title><body>smoke</body>';

const server = createServer(async (request, response) => {
  const path = new URL(request.url ?? '/', 'http://x').pathname;
  if (path === '/' || path === '/app') {
    response.writeHead(200, { 'content-type': 'text/html' }).end(page);
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

const browser = await chromium.launch(process.env.PLAYWRIGHT_CHROMIUM ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM } : {});
let failed = false;
try {
  const tab = await browser.newPage();
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
} finally {
  await browser.close();
  server.close();
}
process.exit(failed ? 1 : 0);
