import { randomUUID } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { createClient, memoryStorage } from '../../src/index.js';
import { URL_, emailLink, signedIn, skip, visitor } from './setup.js';

describe.skipIf(skip)('auth against the server', () => {
  it('signs up, reads the user, signs in again', async () => {
    const { client, email, password, id } = await signedIn();
    expect((await client.auth.getUser()).data).toMatchObject({ id, email });
    const again = visitor();
    const { data, error } = await again.auth.signInWithPassword({ email, password });
    expect(error).toBeNull();
    expect(data?.user.id).toBe(id);
    const wrong = await visitor().auth.signInWithPassword({ email, password: 'nope-nope-nope' });
    expect(wrong.error?.status).toBe(400);
    const duplicate = await visitor().auth.signUp({ email, password });
    expect(duplicate.error).toMatchObject({ status: 409, code: 'user_already_exists' });
  });

  it('rotates refresh tokens, and a reused one ends the session (as the server enforces)', async () => {
    const { client } = await signedIn();
    const first = (await client.auth.getSession()).data!;
    const second = await client.auth.refreshSession();
    expect(second.error).toBeNull();
    expect(second.data!.refresh_token).not.toBe(first.refresh_token);

    // Another client replaying the old token: the server ends the session.
    const replay = visitor();
    await replay.auth.setSession(first);
    expect((await replay.auth.refreshSession()).error?.code).toBe('invalid_grant');
    expect((await replay.auth.getSession()).data).toBeNull();
    expect((await client.auth.refreshSession()).error?.code).toBe('invalid_grant');
  });

  it('many concurrent requests near expiry refresh once and keep the session alive', async () => {
    const { client } = await signedIn();
    const session = (await client.auth.getSession()).data!;
    await client.auth.setSession({ ...session, expires_at: Math.floor(Date.now() / 1000) + 5 });
    const results = await Promise.all([1, 2, 3, 4, 5, 6].map(() => client.rpc('whoami')));
    for (const { error } of results) expect(error).toBeNull();
    const after = (await client.auth.getSession()).data!;
    expect(after.refresh_token).not.toBe(session.refresh_token);
    expect((await client.auth.refreshSession()).error).toBeNull();
  });

  it('two tabs sharing storage refresh one at a time', async () => {
    const storage = memoryStorage();
    const tab = () => createClient(URL_, { auth: { storage, autoRefresh: false } });
    const a = tab();
    const email = `sdk-${randomUUID()}@example.com`;
    await a.auth.signUp({ email, password: 'pw-shared-tabs-1' });
    const session = (await a.auth.getSession()).data!;
    await a.auth.setSession({ ...session, expires_at: Math.floor(Date.now() / 1000) + 5 });
    const b = tab();
    const [ra, rb] = await Promise.all([a.auth.getSession(), b.auth.getSession()]);
    expect(ra.error).toBeNull();
    expect(rb.error).toBeNull();
    expect(ra.data?.refresh_token).toBe(rb.data?.refresh_token);
    expect((await b.auth.refreshSession()).error).toBeNull();
  });

  it('sign-out revokes the refresh token on the server', async () => {
    const { client } = await signedIn();
    const session = (await client.auth.getSession()).data!;
    expect((await client.auth.signOut()).error).toBeNull();
    expect((await client.auth.getSession()).data).toBeNull();
    const replay = visitor();
    await replay.auth.setSession(session);
    expect((await replay.auth.refreshSession()).error?.code).toBe('invalid_grant');
  });

  it('signs in with a magic link from a real email', async () => {
    const { email, id } = await signedIn();
    const nelcota = visitor();
    expect((await nelcota.auth.sendMagicLink(email)).error).toBeNull();
    const link = nelcota.auth.readEmailLink(await emailLink(email, 'https://app.example.com/signed-in'));
    expect(link?.type).toBe('magiclink');
    const { data, error } = await nelcota.auth.verifyEmailLink({ type: 'magiclink', token: link!.token });
    expect(error).toBeNull();
    expect(data?.user.id).toBe(id);
    const reused = await visitor().auth.verifyEmailLink({ type: 'magiclink', token: link!.token });
    expect(reused.error?.status).toBeGreaterThanOrEqual(400);
  });

  it('resets a password from a recovery email', async () => {
    const { email } = await signedIn();
    const nelcota = visitor();
    expect((await nelcota.auth.requestPasswordReset(email)).error).toBeNull();
    expect((await nelcota.auth.requestPasswordReset(`nobody-${randomUUID()}@example.com`)).error).toBeNull();
    const link = nelcota.auth.readEmailLink(await emailLink(email, 'https://app.example.com/new-password'));
    expect(link?.type).toBe('recovery');
    const reset = await nelcota.auth.resetPassword({ token: link!.token, password: 'a-brand-new-password' });
    expect(reset.error).toBeNull();
    expect((await visitor().auth.signInWithPassword({ email, password: 'a-brand-new-password' })).error).toBeNull();
  });

  it('OAuth: unconfigured providers and bogus codes come back as errors', async () => {
    const nelcota = visitor();
    const { data } = await nelcota.auth.signInWithOAuth({ provider: 'github', redirectTo: 'https://app.example.com/back', redirect: false });
    const response = await fetch(data!.url, { redirect: 'manual' });
    expect(response.status).toBeGreaterThanOrEqual(400);
    const exchange = await nelcota.auth.exchangeCode('not-a-real-code');
    expect(exchange.error?.status).toBe(400);
  });
});
