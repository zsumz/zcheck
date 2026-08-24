# Changelog

All notable changes to zcheck are documented in this file.

## Unreleased

## 0.0.2 - 2026-08-24

- Adds protected native binary releases with deterministic archives, clean
  GNU and musl runtime checks, SHA-256 manifests, and GitHub attestations.

## 0.0.1 - 2026-08-23

- Defines strict schema-1 manifests and deterministic shared-dependency plans.
- Runs serial or bounded-parallel task graphs without an implicit shell.
- Preflights applicable executables and declared tools before execution.
- Preserves or requires clean Git state with exact repository fingerprints.
- Writes complete task logs and versioned qualification receipts.
- Contains task trees with timeouts and Ctrl-C handling on Unix and Windows.
- Supports named resource locks, fail-fast scheduling, and independent branches.
- Reports stable human, JSON, and GitHub output with stable process exit classes.
- Exposes non-executing manifest, plan, and receipt contracts through
  `zcheck-core`.
- Verifies publishable crate archives, setup-action behavior, schema fixtures,
  and cross-platform repository scenarios through the repository's own
  `zcheck.toml` graph.
