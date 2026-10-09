/** PKCE (RFC 7636) with S256, on Web Crypto. */

import { base64UrlEncode } from '../core/jwt.js';

/** 64 characters from 48 random bytes (the RFC allows 43 to 128). */
export function createVerifier(): string {
  return base64UrlEncode(crypto.getRandomValues(new Uint8Array(48)));
}

export async function challengeFor(verifier: string): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(verifier));
  return base64UrlEncode(new Uint8Array(digest));
}
