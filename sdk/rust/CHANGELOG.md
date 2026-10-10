# Changelog

## 0.1.0 (unreleased)

- `auth::FileStorage` persists the session in a JSON file, replaced
  atomically and created with mode 0600 on Unix.

- `ClientBuilder::header(name, value)` sends a header with every request.

- **Breaking:** `UploadOptions::content_type` is an `Option<String>`. `None`
  (the default) guesses the type from the object name's extension for common
  types (`storage::guess_content_type`) instead of always sending
  `application/octet-stream`, which made browsers download SVGs and PDFs
  rather than display them.

- **Breaking:** `StorageClient::update_bucket` takes a `BucketUpdate` instead
  of `BucketSettings`. `BucketSettings { public: true, ..Default::default() }`
  silently cleared the size and MIME limits, because the PUT replaces every
  setting. `BucketUpdate` has no `Default`; build it from the current bucket
  (`BucketUpdate { public: true, ..bucket.into() }`) or spell out each field.
  This matches the TypeScript client, which requires all three fields.

- `Error::Http` gains a `db` field with the optional Postgres fields the server
  may send (`sqlstate`, `details`, `hint`, `constraint`), read through
  `Error::sqlstate()` and friends. Code that builds `Error::Http` by hand must
  add `db: None`. New helpers: `is_not_found`, `is_conflict`,
  `is_unique_violation`, `is_rate_limited`, `is_unauthorized`, `is_forbidden`,
  `is_retryable`.
- Error responses without a JSON body use `unauthorized`, `forbidden`,
  `not_found`, `rate_limited` or `unavailable` instead of `http_<status>`.

- Correct negated leaf condition encoding and double negation for leaves and
  groups, with production HTTP/Postgres regressions.

- Native Tokio client with standalone rustls/ring HTTPS, structured errors,
  request cancellation, bounded read retries and deadlines.
- REST reads/writes, groups, embeds, count, cardinality, upsert and RPC.
- In-memory/custom session stores, coordinated refresh, auth events,
  optional background refresh, email links and OAuth PKCE.
- Streaming storage, ranges/ETags, signed/public URLs and bucket management.
- Rust catalog models through `nelcota types --lang rust`, with precise numeric
  values and explicit omitted/null write fields.
