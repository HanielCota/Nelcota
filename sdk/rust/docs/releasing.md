# Releasing nelcota-client

The Rust package has independent semver. A release targets the documented
server protocol, rather than requiring matching server/package versions.

Before the first release, confirm ownership/availability of `nelcota-client`
on crates.io. Configure the GitHub `crates-io` environment with a scoped
`CARGO_REGISTRY_TOKEN` permitted to publish this crate.
This repository does not contain registry credentials.

1. Run SDK checks and `cargo test -p nelcota-server --test sdk_rust` with Docker.
2. Run the Rust generator tests and external package consumer check.
3. Update the package version, compatibility contract and changelog.
4. Run `cargo publish -p nelcota-client --dry-run` from a clean checkout.
5. Push an intentional `rust-sdk-v<version>` tag to invoke sdk-rust-release.yml.

The release workflow checks tag/version equality, repeats SDK checks and the
real HTTP/Postgres contracts, verifies the package and publishes in the
`crates-io` environment. Publishing requires maintainer registry configuration;
local implementation/testing does not publish or create tags.

Example external package check (after `cargo package -p nelcota-client`):

```sh
python sdk/rust/scripts/package-test.py
```

To intentionally verify default HTTPS outside the workspace:

```sh
cargo run -p nelcota-client --example https_smoke
```

That makes one anonymous GET to example.com; an HTTP response proves TLS
initialized in the consumer process without Nelcota server startup.
