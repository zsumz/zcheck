# Publishing zcheck

Publication is a manual, authority-gated operation. This document records the
order and evidence required; it does not authorize publishing, tagging, pushing,
or changing package versions.

## Release scope

`0.0.1` is the crates-only initial release. Native binary archives begin with
`0.0.2`:

- Publish `zcheck-core`, then `zcheck`, to crates.io.
- Publish binary archives only after both crates pass registry verification.
- `.github/actions/setup-zcheck` is a tested contract for exact archives with
  caller-reviewed digests; it is not a published action.
- Schema 1 is the stable `0.0.1` contract. Incompatible changes require a new
  schema number and updated golden contracts.

## Binary releases

Signed stable version tags publish native archives through
`.github/workflows/release.yml`. Repository rules protect `v*` tags, and the
`release` environment requires human review and accepts deployments only from
those protected tags.

The workflow rejects tags that do not exactly match the workspace version or
are not reachable from the default branch. It then runs the complete offline
qualification graph before building this exact release set:

- x86-64 and ARM64 Linux with GNU libc;
- x86-64 and ARM64 Linux with musl;
- x86-64 and ARM64 macOS;
- x86-64 Windows with MSVC.

Every archive contains only the `zcheck` executable, `LICENSE`, and `README.md`
with deterministic archive metadata. GNU and musl binaries run a qualification
graph in clean Ubuntu and Alpine containers without a Rust toolchain. The
publish job requires all seven archives, verifies `SHA256SUMS`, creates GitHub
provenance attestations, and publishes the complete release from a draft only
after every preceding job passes.

Release notes come from the exact version section in `CHANGELOG.md`. A tag must
not be pushed until that reviewed section exists and the crates.io artifacts for
the same version have passed the registry-only installation check below.

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

Immediately before publication, query the exact crates.io endpoints for both
package names and confirm that the candidate version does not already exist:

```sh
curl -sS -o /dev/null -w '%{http_code}\n' https://crates.io/api/v1/crates/zcheck-core
curl -sS -o /dev/null -w '%{http_code}\n' https://crates.io/api/v1/crates/zcheck
```

Both responses must be reviewed before packaging or uploading either crate.

## Publish in dependency order

From the exact qualified commit, first rehearse and publish the public core:

```sh
cargo publish --dry-run --locked --registry crates-io -p zcheck-core
cargo publish --locked --registry crates-io -p zcheck-core
```

Wait until crates.io exposes the exact candidate `zcheck-core` version. Then
rehearse the CLI against the registry dependency and publish it:

```sh
cargo publish --dry-run --locked --registry crates-io -p zcheck
cargo publish --locked --registry crates-io -p zcheck
```

Each real `cargo publish` command requires fresh explicit publication authority.

## Verify what users receive

Use a fresh Cargo home and install the exact registry version:

```sh
ZCHECK_VERSION=0.0.2
ZCHECK_VERIFY_HOME="$(mktemp -d)"
CARGO_HOME="$ZCHECK_VERIFY_HOME/cargo" \
  cargo install zcheck --version "$ZCHECK_VERSION" --locked --registry crates-io
"$ZCHECK_VERIFY_HOME/cargo/bin/zcheck" --version
```

The final command must report the exact candidate version. Preserve the hosted
job links, crate-version pages, package checksums, and installation transcript
as release evidence before any tag or announcement.
