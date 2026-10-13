# Changelog

Server releases are listed here. The [TypeScript SDK](sdk/typescript/CHANGELOG.md)
and [Rust SDK](sdk/rust/CHANGELOG.md) have independent package versions.

## Unreleased

### Added

- A [complete Notes staging guide](docs/onboarding/notes.md) and isolated
  native acceptance runner covering installation, migrations, generated types,
  panel login, two-user RLS, private files, logout and restart persistence.
- Reusable auth/core package boundaries, scoped database bootstrap and an
  external archive consumer; integration and release evidence under `docs/goals/`.

### Fixed

- Allow installation into a writable user-owned `PREFIX` without sudo while
  preserving checksum verification before replacing the existing binary.
- Bound password hashing workers and refill completed webhook delivery slots
  promptly. Rotate refresh sessions with one database statement.
- Reject refresh lifetimes outside PostgreSQL's timestamp range before
  persisting tokens.
- Index user and storage directory listings, cap panel pagination inputs and
  remove cyclic imports in the embedded panel.

### Upgrade notes

- Startup applies internal migrations `V14__user_listing_index.sql` and
  `V15__storage_directory_index.sql` when upgrading from `0.4.0`. Plan a
  maintenance window for large user or storage metadata tables.
- Keep database backups and their paired disk snapshots before upgrading.
  See [release handoff](docs/onboarding/release.md) for validation and rollback.
- The TypeScript SDK's cross-tab storage publication correction needs a
  separate SDK package release; installing the server does not update clients.
- This section describes an unpublished candidate. The published `0.4.0`
  binaries and image do not contain these changes.

## 0.4.0 — 2026-10-11

### Added

- [Events and webhooks](docs/webhooks.md): capture committed `INSERT`, `UPDATE`
  and `DELETE` changes in PostgreSQL and dispatch signed HTTP deliveries from
  the existing Rust server. Rolled-back writes do not enqueue events.
- Panel configuration, one-time signing keys, key rotation, pause/resume,
  delivery history and manual resends, with English and Portuguese guides.
- Persistent delivery leases, bounded concurrency, retries, recovery after
  restart, queue admission limits and seven-day terminal-history retention.
- Verified HTTPS destinations with connection-time DNS/address checks,
  disabled redirects and proxies, and an explicit loopback-only development
  option. System DNS work stays bounded after request cancellation.

### Fixed

- Bound catalog notifications, project-health probes and SMTP delivery work
  so stalled services cannot create an unbounded backlog or retain permits.
- Limit panel and SDK HTTP error bodies to 64 KiB while preserving HTTP status
  and retry metadata. Bound upload acknowledgements and cancel failed uploads
  without leaving queue work running.
- Preserve rotated or signed-out sessions when an adapter fails to write or
  remove stored state. Observe background auth failures and avoid refreshing
  stale state from another tab.
- Share one Rust SDK refresh worker across cancelled callers and wait for
  aborted webhook worker cleanup during shutdown.
- Generate validators for every webhook response and keep unused organization
  avatar drafts out of the embedded panel.
- Correct SMTP fixtures and quote disposable database names during test cleanup.

### Upgrade notes

- Back up each project before upgrading. Startup applies the new internal
  `V13__webhooks.sql` migration; existing released migrations are unchanged.
- Webhooks require administrator configuration and send full changed rows.
  Receivers must verify signatures and deduplicate event IDs: delivery is at
  least once, with no ordering guarantee.
- Queue-capacity errors roll back the business write rather than lose its
  event. Oversized event payloads are recorded as failed with omitted contents.
- Public destinations require HTTPS. `NELCOTA_WEBHOOKS_ALLOW_LOOPBACK=true`
  permits local development receivers without opening private networks.
- The corrected SDK packages are `@nelcota/client` 0.1.2 and `nelcota-client`
  0.1.2. Their package versions remain independent of the server version.

## 0.3.1 — 2026-10-11

- Publish server binaries and images from the Nelcota organization, using
  `ghcr.io/nelcota/nelcota-server` for generated Docker deployments.
- Update repository, documentation and installer links to `Nelcota/Nelcota`.
- Include embedded English/Portuguese guides, anonymous OpenAPI fixes and
  maintained dependency updates. SDK 0.1.1 releases update organization metadata.
