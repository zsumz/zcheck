# Schema 1 compatibility contract

This document defines the machine contracts emitted and consumed by
`zcheck 0.0.1`. The checked-in golden files under
`crates/zcheck-core/contracts/schema-1` are normative examples and ship in the
`zcheck-core` crate archive.

## Compatibility rule

In `0.0.1`, schema 1 field names, field meanings, JSON shapes, enum spellings,
and exit-code meanings are stable. Unknown fields fail when parsing manifests
and receipts. An incompatible field change requires a new schema number and
matching updates to this document and every checked-in golden contract.

Writers emit `schema = 1` or `"schema": 1`. Readers reject every unsupported
schema number before treating the document as evidence.

## Manifest

`zcheck.toml` has these root fields:

| Field | Requirement | Meaning |
| --- | --- | --- |
| `schema` | required | Must be `1`. |
| `default` | optional | Task selected by bare `zcheck` and `zcheck run`. |
| `execution` | optional | Repository-wide execution defaults. |
| `tasks` | required | Non-empty table of named tasks. |

`execution` accepts only `jobs`, `default_timeout`, and `repository_state`.
Defaults are one job, no timeout, and `preserve`. Repository-state values are
`preserve`, `clean`, and `ignore`.

A task accepts only `description`, `run`, `needs`, `cwd`, `env`, `tools`,
`timeout`, `platforms`, `resources`, and `inputs`. Every task defines `run`,
`needs`, or both. `run` is a non-empty argument array and is never interpreted
by an implicit shell. Task names match `[a-z][a-z0-9_-]*`. Platforms are
`linux`, `macos`, and `windows`. Durations are positive integers followed by
`ms`, `s`, `m`, or `h`.

Unknown dependencies, duplicate graph metadata, cycles, escaping
repository-relative paths, invalid process values, and process-only fields on
aggregate tasks fail validation. Validation covers the complete manifest,
independent of the current host.

## Plan JSON

`zcheck plan TASK --format json` emits:

| Field | Meaning |
| --- | --- |
| `schema` | Plan schema, fixed at `1`. |
| `selection` | Root task names in command-line order. |
| `platform` | The projected platform. |
| `jobs` | Maximum concurrent executable tasks. |
| `tasks` | Reachable tasks in deterministic dependency order. |

Each planned task contains `name`, `description`, `kind`, `applicability`,
`command`, `needs`, `cwd`, `environment`, `tools`, `resources`, `inputs`, and
`timeout`. Kinds are `executable` and `aggregate`; applicability is `applicable`
or `skipped`. Environment contains only explicit variable names, never values.
A skipped branch remains visible, but its private dependencies are not planned.

## Receipt JSON

Every completed run persists one receipt containing `schema`, `run_id`,
`runner`, `manifest`, `repository`, `selection`, `host`, `status`, `logs_dir`,
`receipt`, and plan-ordered `tasks`.

The overall statuses are `passed`, `failed`, and `cancelled`. Task statuses are
`passed`, `failed`, `blocked`, `skipped`, and `cancelled`. A task result contains
`name`, `status`, `command`, `cwd`, `duration_ms`, `exit_code`, `log`, `reason`,
`environment`, and `termination`. Termination reasons are `timeout` and
`interrupted`, paired with a `forced` boolean.

Repository evidence records the requested state mode, before and after Git
snapshots, and the preservation result. Each snapshot records its combined
digest, HEAD identity, staged, unstaged, untracked, and submodule digests, and
cleanliness. Inherited environment values are never recorded.

`Receipt::parse_json` is the version-validating reader. `Receipt::to_json_pretty`
and `Plan::to_json_pretty` are the canonical pretty JSON writers.

## Process exits

| Exit | Meaning |
| ---: | --- |
| `0` | The selected graph passed. |
| `1` | Qualification failed, blocked, timed out, or changed repository state. |
| `2` | CLI usage or manifest configuration was invalid. |
| `3` | The zcheck runner failed internally. |
| `130` | The user interrupted the run where conventional signal status applies. |

Child-process exit values remain evidence inside task results. They never
replace the stable zcheck process classification.

## Static consumers

`zcheck-core` is the reusable, non-executing analysis boundary. A static tool
such as zrail parses source with `Manifest::parse`, enumerates `Manifest::tasks`,
reads every task field through public getters, and resolves platform-specific
graphs with `Manifest::plan`. It does not depend on scheduler or CLI internals.
