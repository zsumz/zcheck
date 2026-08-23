<p align="center">
  <img src="https://raw.githubusercontent.com/zsumz/zcheck/main/zcheck-logo.svg" alt="zcheck" width="640">
</p>

<p align="center"><strong>One manifest and one command give you a trustworthy answer to whether the checkout is good.</strong></p>

<p align="center">
  zcheck runs a repository's qualification graph, preserves the checkout, and records durable evidence.
</p>

<p align="center">
  <a href="#install">Install</a>
  <span> · </span>
  <a href="#quick-start">Quick start</a>
  <span> · </span>
  <a href="#manifest">Manifest</a>
  <span> · </span>
  <a href="#evidence">Evidence</a>
  <span> · </span>
  <a href="#qualification">Qualification</a>
</p>

## Install

```sh
cargo install zcheck --version 0.0.1 --locked
```

Building from source requires Rust 1.96 or newer. The canonical toolchain is
pinned in `rust-toolchain.toml`.

## Quick start

Add `zcheck.toml` at the repository root:

```toml
schema = 1
default = "check"

[tasks.format]
description = "Validate formatting."
run = ["cargo", "fmt", "--all", "--", "--check"]
tools = ["cargo"]

[tasks.test]
description = "Run tests."
run = ["cargo", "test", "--locked"]
tools = ["cargo"]

[tasks.check]
description = "Qualify the checkout."
needs = ["format", "test"]
```

Run the default task:

```sh
zcheck
```

Useful commands:

```sh
zcheck plan check
zcheck run test -- --nocapture
zcheck list
zcheck validate
zcheck doctor
```

zcheck searches upward from the current directory for `zcheck.toml`.
`--root` and `--manifest` select a repository or manifest explicitly.

## What zcheck provides

- Deterministic dependency planning and bounded parallel execution.
- Direct argument-vector commands without an implicit shell.
- Whole-graph tool preflight and invocation-local resource locks.
- Complete stdout and stderr logs for every executable task.
- Timeouts and Ctrl-C handling for complete process trees.
- Stable human, JSON, and GitHub reporting.
- Versioned receipts with manifest, host, task, artifact, and Git evidence.
- Exact `preserve`, `clean`, and `ignore` repository-state policies.

A failed task blocks its descendants while independent branches continue.
Platform-specific branches remain visible as skipped instead of claiming proof
for another operating system.

## Manifest

Every task runs a process, depends on other tasks, or does both. A task with
dependencies and no command is an aggregate. Shared dependencies execute once
per invocation.

```toml
schema = 1
default = "check"

[execution]
jobs = 4
default_timeout = "20m"
repository_state = "preserve"

[tasks.package]
description = "Verify package contents."
run = ["scripts/package-check"]
tools = ["cargo", "tar"]

[tasks.check]
description = "Qualify the checkout."
needs = ["package"]
```

Pipelines, loops, redirection, and substantial environment handling belong in
checked-in programs such as `scripts/package-check`. Unknown fields,
dependencies, cycles, invalid names, and escaping repository paths fail
validation.

[Schema 1](https://github.com/zsumz/zcheck/blob/main/docs/schema-1.md) defines
the complete manifest, plan, receipt, status, and compatibility contracts.

## Evidence

Each completed run writes a schema-1 receipt and complete task logs outside the
checkout by default. CI can select explicit artifact locations:

```sh
zcheck run check \
  --format github \
  --logs-dir "$RUNNER_TEMP/zcheck/logs" \
  --receipt "$RUNNER_TEMP/zcheck/receipt.json"
```

Repository-state policy is part of the result:

| Mode | Contract |
| --- | --- |
| `preserve` | Allow initial dirt and require the exact tracked, staged, untracked, symlink, and submodule state afterward. |
| `clean` | Require a clean checkout before and after qualification. |
| `ignore` | Do not inspect Git state. |

Process exit codes are stable: `0` passed, `1` qualification failed or was
blocked, `2` usage or configuration was invalid, `3` the runner failed, and
`130` the user interrupted the run.

## Crates

| Crate | Purpose |
| --- | --- |
| `zcheck` | Discovery, Git inspection, bounded DAG execution, logs, and reporting |
| `zcheck-core` | Public manifest, validation, platform, plan, and receipt contracts |
| `zcheck-testkit` | Unpublished adversarial repositories and cross-platform fixtures |

`zcheck-core` lets static consumers inspect qualification graphs without
executing repository code. zrail can review `zcheck.toml` and declared inputs;
zcheck can run `zrail check` as one task. Neither crate depends on the other.

## Qualification

```sh
cargo fetch --locked
cargo run --locked --offline -p zcheck -- run check
```

The checked-in `zcheck.toml` is the complete local qualification graph. There
is no `scripts/check`; scripts contain implementation logic, while simple
commands stay in the manifest.

See the [design](https://github.com/zsumz/zcheck/blob/main/docs/design.md) for
trust boundaries and execution semantics, and the
[publishing procedure](https://github.com/zsumz/zcheck/blob/main/docs/publishing.md)
for the authority-gated release checklist.

## Scope

zcheck qualifies finite repository checks. It does not replace a build system,
analyze architecture, install toolchains, manage secrets, deploy software,
generate CI, run dev servers, or resolve remote task packages.

## License

Apache-2.0. See [LICENSE](https://github.com/zsumz/zcheck/blob/main/LICENSE).
