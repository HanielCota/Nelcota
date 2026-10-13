# Notes acceptance evidence — 2026-10-13

The documented installation path was exercised against both the published
Linux amd64 `0.4.0` binary and a rebuilt binary from this checkout. Both report
`0.4.0` because the candidate's server version has not yet been bumped; their
SHA-256 values distinguish the actual artifacts.

| Artifact | Evidence | Result |
| --- | --- | --- |
| Published `0.4.0` | [Native installation](results/notes-published-0.4.0.json) | Six native check groups passed; installer verified SHA-256 |
| Checkout, pass 1 | [Native + browser](results/notes-candidate-pass-1.json) | Seven check groups passed |
| Checkout, pass 2 | [Native + browser](results/notes-candidate-pass-2.json) | Seven check groups passed on another fresh database |

The two final candidate passes used the same rebuilt binary, PostgreSQL
17.11 and system Chromium. The second ran from `sdk/typescript` using the
relative binary path used by SDK CI. The checks exercised the real native
CLI, rather than applying application SQL directly through a test harness:

- Start `nelcota dev`, verify database health and log in to the embedded panel.
- Apply `examples/notes.sql` through `migrate`, repeat it without reapplying,
  and generate TypeScript and Rust schema types.
- Create two accounts, sign in with passwords, perform Notes CRUD, reject a
  forged owner and prevent reads, updates or deletes across users.
- Upload/download exact file bytes and prevent anonymous or cross-user reads
  and overwrites.
- Log out and reject the retired refresh token.
- Stop/restart the API and PostgreSQL, preserving account identity, notes,
  file bytes, private credentials and signing keys.
- In Chromium, use the shipped browser example for signup, password login,
  save/list, upload, independent-user RLS and logout across another origin.

Every successful run removed its own database container and data volume. A
separate real cancellation check sent SIGTERM after startup: the runner exited
non-zero, recorded failure and removed its owned database container/volume.
Pre-existing development and shared test databases were preserved.

Supporting checks passed: 36 Python review-tool regressions (including 11 new
installer, ownership, redirect, response-size and child-process checks), 33
panel guide tests (including the external Notes guide link), Portuguese guide
consistency, panel/server builds and panel type/diagnostic checks,
TypeScript SDK type check, native Oxlint base/import presets with zero findings,
capability boundaries, shell syntax, workflow YAML and documentation links.

The native and browser acceptance steps are configured in CI, with only
sanitized JSON reports retained as artifacts. The local results above were
recorded before the PR's CI execution; see [PR #83](https://github.com/Nelcota/Nelcota/pull/83)
for its workflow results. This evidence does not establish a public
VPS deployment, DNS/TLS, SMTP/OAuth, S3 acceptance or a new sustained-load soak.
See the [release handoff](release.md) for those separate gates.
