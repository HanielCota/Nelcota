# Rust SDK contract

`nelcota-client` 0.1.x targets the current repository HTTP protocol (workspace
version 0.2.0), rather than every binary carrying that version. The released
v0.2.0 tag is a legacy baseline without the newer OAuth routes and generators;
see the README compatibility matrix. Package and server versions are
independent. Native Tokio on Linux, Windows and macOS is
the supported target; blocking and WASM interfaces are not part of 0.1.

| Module | HTTP contract |
|---|---|
| REST read | GET/HEAD `/rest/v1/{table}`; select/filter/order/paging, Prefer count, Content-Range |
| REST write | POST/PATCH/DELETE `/rest/v1/{table}`; minimal/representation, upsert resolution/on_conflict |
| RPC | POST `/rest/v1/rpc/{name}` with named JSON arguments |
| Auth | POST signup; token grants password/refresh_token/pkce; GET user; POST logout |
| Email | POST recover/magiclink/resend/verify |
| OAuth | Authorization URL with S256 PKCE; one-use code exchange |
| Objects | POST/PUT/GET/DELETE `/storage/v1/object/{bucket}/{path}`, raw bytes |
| Sharing/listing | POST object/list and object/sign; local object/public URL construction |
| Buckets | GET/POST bucket; GET/PUT/DELETE bucket/{id} |

The query grammar and server capabilities match the TypeScript contract.
Responses become Rust Result values; empty bodies become JSON null (decode to
unit or an appropriate Option). `Response<T>` carries status, count and headers.
Auth/storage convenience methods return the decoded model or structured error.

One Client and its clones share lifecycle coordination. Explicit token clones
control REST/storage and get_user only; other auth methods manage the shared
login session. Per-request cancellation does not stop an accepted rotation.
Default sessions never persist outside memory. Shared stores across independently
constructed clients/processes require their own coordination.

Generated models provide serde shapes, not compile-time verification of string
queries. Missing required selected columns fail decoding. Numeric precision is
preserved by arbitrary_precision; write null and omitted fields differ.

Acceptance checks: SDK tests/doctests/examples, server HTTP contract tests,
Rust generator snapshot and generated model compile/serialization tests,
Clippy/rustfmt, docs, package verification and an external consumer smoke test.
