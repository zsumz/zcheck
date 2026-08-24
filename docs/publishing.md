# Publishing zcheck

Publication is a manual, authority-gated operation. This document records the
order and evidence required; it does not authorize publishing, tagging, pushing,
or changing package versions.

## Release scope

`0.0.1` is crates-first:

- Publish `zcheck-core`, then `zcheck`, to crates.io.
- Binary archives and the setup action are outside this release.
- `.github/actions/setup-zcheck` is a tested contract for exact archives with
  caller-reviewed digests; it is not a published action.
- Schema 1 is the stable `0.0.1` contract. Incompatible changes require a new
  schema number and updated golden contracts.

## Required evidence

Before requesting publication authority, record all of the following for the
exact release commit:

- A clean canonical Ubuntu qualification.
- Passing macOS and Windows portable test jobs.
- Passing Rust 1.96.1 compatibility tests.
- Package verification from extracted crate archives.
- A passing security advisory job.
- A clean zrail review with no grants, debt increases, or unknown changes.
- A clean local worktree and a verified PGP-signed release commit.

Do not replace hosted matrix evidence with a single-machine build.

## Fresh registry checks

An exact crates.io lookup on 2026-08-23 found no published package named
`zcheck`. That historical result does not reserve the name and does not replace
fresh checks for both `zcheck` and `zcheck-core`.

Immediately before first publication, query the exact crates.io endpoints for
both package names:

```sh
curl -sS -o /dev/null -w '%{http_code}\n' https://crates.io/api/v1/crates/zcheck-core
curl -sS -o /dev/null -w '%{http_code}\n' https://crates.io/api/v1/crates/zcheck
```

Both responses must be reviewed. For an unclaimed name the API returns `404`;
any other result stops the first-publication procedure for investigation.

## Publish in dependency order

From the exact qualified commit, first rehearse and publish the public core:

```sh
cargo publish --dry-run --locked --registry crates-io -p zcheck-core
cargo publish --locked --registry crates-io -p zcheck-core
```

Wait until crates.io exposes `zcheck-core 0.0.1`. Then rehearse the CLI
against the registry dependency and publish it:

```sh
cargo publish --dry-run --locked --registry crates-io -p zcheck
cargo publish --locked --registry crates-io -p zcheck
```

Each real `cargo publish` command requires fresh explicit publication authority.

## Verify what users receive

Use a fresh Cargo home and install the exact registry version:

```sh
ZCHECK_VERIFY_HOME="$(mktemp -d)"
CARGO_HOME="$ZCHECK_VERIFY_HOME/cargo" \
  cargo install zcheck --version 0.0.1 --locked --registry crates-io
"$ZCHECK_VERIFY_HOME/cargo/bin/zcheck" --version
```

The final command must report `zcheck 0.0.1`. Preserve the hosted job links,
crate-version pages, package checksums, and installation transcript as release
evidence before any tag or announcement.
