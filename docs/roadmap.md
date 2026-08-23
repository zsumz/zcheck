# Roadmap

Version `0.0.1` contains six delivered capability slices. Milestone numbers
describe the product model, not package versions.

## 1. Manifest and plan

- Strict schema-1 parsing and unknown-field rejection.
- Task-name, dependency, cycle, duration, and path validation.
- Platform projection and deterministic shared-dependency planning.
- Human and versioned JSON `list` and `plan` output.
- kafkars, zrail, zhold, Rafter, and Zolt manifest fixtures.

## 2. Trustworthy execution

- `zcheck`, `zcheck run`, `validate`, and `doctor` commands.
- Direct argument-vector processes, working directories, and environment
  overrides.
- Whole-graph tool preflight, complete task logs, stable statuses, JSON reports,
  and stable exit codes.
- Fail-closed manifest containment, branch-local working-directory blocking,
  private portable logs, and extracted-archive self-tests.

## 3. Repository evidence

- Exact Git-state fingerprints and `preserve`, `clean`, and `ignore` policies.
- Manifest digests, stable receipts, cache-directory defaults, and safe
  environment metadata.

## 4. Process handling

- Unix process groups and Windows Job Objects.
- Timeouts, Ctrl-C, escalation, and deadlock-safe streaming output capture.

## 5. DAG concurrency

- Bounded jobs, deterministic readiness, and named resources.
- Independent continuation, fail-fast scheduling, and concurrent reporting.

## 6. CI evidence

- GitHub groups and bounded annotations.
- Explicit artifact paths and immutable artifact retention.
- A tested exact-release setup-action contract.

## Schema 1

- Manifest, plan, and receipt schema 1 have checked-in golden contracts.
- `zcheck-core` exposes version constants, deterministic JSON writers, and a
  version-validating strict receipt reader.
- The external-consumer suite proves the complete static analysis surface.
- Stable exit codes are compatibility-tested through both the library boundary
  and the `zcheck` binary.
- Minimum-Rust CI executes the core compatibility suite.
- Packaged core archives carry and execute the same golden contracts.

## Integration surface

Representative manifests cover kafkars, zrail, zhold, Rafter, and Zolt.
`zcheck-core` provides the non-executing analysis boundary for tools such as
zrail, while `zcheck` executes the repository's declared graph.
