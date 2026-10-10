# Changelog

## Unreleased

- Expose the optional Postgres fields of server errors (`sqlstate`, `details`,
  `hint`, `constraint`) on `NelcotaError`, and export `NelcotaErrorCode` and
  `ServerErrorCode` for autocompletion of known codes.
- Bodyless error responses (HEAD counts) keep a meaningful code:
  `unauthorized`, `forbidden`, `not_found`, `rate_limited` or `unavailable`
  instead of `http_<status>`.

- Cancel auth/request waits independently while accepted refresh rotation
  completes and persists the replacement token.
- Type HEAD data as null and require row representation for single/maybeSingle;
  reject malformed REST row representations with invalid_response.
- Require complete bucket settings on update, with explicit null to clear
  limits. Existing partial updateBucket calls must supply all three fields.
- Add regressions for cancellation, result types, cardinality and bucket limits
  against both mock transports and the production HTTP/Postgres contract.

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
