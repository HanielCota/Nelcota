# Changelog

## 0.1.0 — Initial release candidate

- JavaScript ESM and TypeScript declarations with no runtime dependencies.
- REST queries, writes, upserts, filters, embeds, pagination, counts and RPC.
- Password, email-link and OAuth PKCE authentication; persistent browser
  sessions and coordinated refresh rotation.
- Storage uploads, downloads, streaming responses, ranges, ETags and URLs.
- Independent `createRestClient`, `createAuthClient` and `createStorageClient`
  factories, with a caller-supplied token source for sharing auth.
- Buffered response failures return structured errors; read retries include
  interrupted bodies and never retry earlier than `Retry-After`.
- Auth request cancellation, logout/refresh coordination and client disposal.
- Session keys isolate projects served under different URL base paths.
- Packed-package checks in clean JavaScript and TypeScript consumers, contract
  tests against Postgres and Nelcota, and browser/runtime checks.
- Runnable browser, Node and per-request server examples; Portuguese quickstart.

The candidate becomes a public release only after a successful npm publish.
