# Security

## Reporting a vulnerability

Please **do not open a public issue**. Report it privately through
[GitHub's private vulnerability reporting](https://github.com/HanielCota/Nelcota/security/advisories/new)
(Security → Report a vulnerability).

Include what you can: the affected version or commit, how to reproduce it,
and what an attacker gains. You will get an answer within a few days; once a
fix is out, the advisory is published with credit to you, unless you prefer
otherwise.

## Supported versions

Only the latest release gets fixes. Upgrade with `nelcota upgrade`.

## Scope

Nelcota's security model is in [docs/jwt-and-roles.md](docs/jwt-and-roles.md)
and [docs/storage.md](docs/storage.md): authorization belongs to RLS, so a
way to read or change data that the policies should forbid, to assume a role
a token does not carry, or to reach the panel without its login is in scope.
So is anything that leaks secrets (keys, password hashes, tokens) or lets an
uploaded file run code in a browser.

A `service_role` token bypasses RLS by design; one leaked from a frontend is
a misconfiguration, not a vulnerability in Nelcota.

## Verifying a release

Release binaries and images carry signed build provenance:

```sh
gh attestation verify nelcota-x86_64-unknown-linux-musl -R HanielCota/Nelcota
gh attestation verify oci://ghcr.io/hanielcota/nelcota-server:<version> -R HanielCota/Nelcota
```
