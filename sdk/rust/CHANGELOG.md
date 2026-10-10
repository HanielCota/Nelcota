# Changelog

## 0.1.0 (unreleased)

- Native Tokio client with standalone rustls/ring HTTPS, structured errors,
  request cancellation, bounded read retries and deadlines.
- REST reads/writes, groups, embeds, count, cardinality, upsert and RPC.
- In-memory/custom session stores, coordinated refresh, auth events,
  optional background refresh, email links and OAuth PKCE.
- Streaming storage, ranges/ETags, signed/public URLs and bucket management.
- Rust catalog models through `nelcota types --lang rust`, with precise numeric
  values and explicit omitted/null write fields.
