# Glossary

Use these terms when reading guides or reporting a problem. You need no running
installation to use this reference.

| Term | Meaning |
| --- | --- |
| Host | Deployment directory and machine managed by the CLI, with shared proxy and project registry. |
| Project | Isolated application deployment with its own PostgreSQL, users, keys and storage. Docker supports several projects; systemd supports one per host. |
| Schema | PostgreSQL namespace. The configured exposed schema generates REST resources; internal schemas hold auth/storage/control data. |
| Role | PostgreSQL identity used for privileges. A validated JWT selects a closed caller role within a transaction. |
| GRANT | SQL permission to use an object. RLS does not grant access by itself. |
| RLS | Row-level security. PostgreSQL filters/checks rows using policies for the active role. |
| Policy | SQL `USING`/`WITH CHECK` rule for permitted rows and operations. |
| `service_role` | Trusted server role with `BYPASSRLS`, still subject to SQL privileges. Never expose its token to clients. |
| Session | Login identity and rotating refresh-token family. Revoking refresh does not retract issued access JWTs. |
| Access token | Signed bearer JWT identifying a caller until expiry. |
| Refresh token | Opaque credential exchanged for a new token pair. Store privately and serialize rotation. |
| Bucket | Storage configuration grouping object names and visibility/MIME/size limits. |
| Object | Metadata row under RLS plus an immutable version of file bytes on disk/S3. |
| Migration | Versioned SQL file with an immutable applied checksum. Application and internal migrations have separate histories. |
| Event / delivery | Committed row change / one destination's attempt history. Webhooks are at least once. |
| Backup pair | Database dump and matching disk-file snapshot/manifest. Keep both. |
| PITR | Database recovery from a base backup and archived WAL; file bytes are separate. |
| Operator | Separately enrolled update-controller identity with explicit deployment scope. Project admin login does not grant this authority. |

Related: [documentation](README.md), [JWT/roles](jwt-and-roles.md) and
[security](security/README.md).
