# zcheck design

## Product boundary

zcheck answers what a repository considers good, what will run, what depends on
what, what passed, and what evidence the run produced. It orchestrates existing
tools; it does not replace language build systems or static architecture review.

The complete qualification contract is reviewable from one checkout. zcheck has
no remote includes, task packages, shell-string execution, or runtime language
detection.

## Schema 1

The normative wire and compatibility rules are in
[the schema-1 contract](schema-1.md). This section explains the model.

The root manifest requires `schema`, may select a `default` task, contains one
`execution` policy, and defines a non-empty `tasks` table. Unknown fields fail.

Each task accepts only:

- `description`
- `run`
- `needs`
- `cwd`
- `env`
- `tools`
- `timeout`
- `platforms`
- `resources`
- `inputs`

Every task has `run`, `needs`, or both. `run` is a non-empty argument array.
Task names match `[a-z][a-z0-9_-]*`. Dependencies must exist, and cycles report
the complete closed path. Repository-relative paths may not escape the root.
Process-only fields (`cwd`, `env`, `tools`, `timeout`, and `resources`) require
`run`; aggregates may use description, dependencies, platforms, and inputs.
Process arguments and environment values containing NUL are invalid.

`validate` evaluates the whole manifest independent of the host platform.
`plan` projects the validated graph onto one platform and emits each reachable
task once in dependency order. Platform-excluded branches remain visible as
skipped plan entries. Human plans JSON-quote each argument so direct-vector
boundaries remain visible; JSON plans retain the argument arrays themselves.

Host projection fails on operating systems outside Linux, macOS, and Windows;
an unsupported host is never silently treated as Linux.

## Execution boundary

The executor preflights every applicable executable and its declared tools,
then schedules ready tasks in deterministic plan order. `execution.jobs` bounds
active executable tasks and `--jobs` may override that bound for one run. A task
claims all of its named resources atomically before it starts; a conflicting
task waits while unrelated ready work may proceed. Resource locks are local to
one zcheck invocation. Processes use direct argument vectors, task working
directories, and explicit environment overrides without an implicit shell.
Each executable receives a complete log outside the checkout. Those concerns
remain internal to the CLI and outside `zcheck-core`.

The stable run statuses are `passed`, `failed`, `blocked`, `skipped`, and
`cancelled`. Missing tools block; they never quietly skip. A failed or blocked
dependency blocks an executable descendant, while independent branches continue.
Aggregates inspect all applicable descendants so a deeper failure is not hidden
by an intermediate blocked task. Platform-skipped branches do not cause their
private dependencies to be planned or preflighted. `--fail-fast` stops
scheduling after the first failed or blocked result, allows active tasks to
finish, and marks otherwise-runnable unscheduled work blocked. It remains a
qualification failure with exit `1`, not a user cancellation.

Before the manifest is parsed, its resolved path must remain within the selected
repository root. Before any task starts, executable working directories and
repository-local programs are resolved against that checkout. A missing path
blocks its task while independent branches continue, but
a working-directory or executable symlink that resolves outside the repository
is a configuration error. Processes receive the resolved executable and working
directory captured by preflight rather than repeating a mutable relative lookup.
An executable found through an absolute `PATH` entry is an external tool. One
found through a relative `PATH` entry is repository-relative and must pass the
same canonical containment check as an explicitly named local executable.

Every task runs in a dedicated Unix process group or Windows Job Object. stdout
and stderr are drained concurrently in bounded chunks into the complete task
log, without retaining whole streams in memory. The runner does not declare a
task complete while descendants remain alive, even when the direct child has
already exited. On Linux, process-table inspection distinguishes executable
group members from dead zombies left behind by a non-reaping container PID 1;
a dead-only group cannot make task termination wait forever.

An effective task timeout fails the task and terminates its complete process
tree. On Unix, zcheck sends `SIGTERM`, waits a fixed grace period, and escalates
to `SIGKILL` if necessary. Windows has no general graceful signal for an
arbitrary process tree, so zcheck terminates the Job Object and records
`forced = true`. Ctrl-C stops new scheduling, applies the same tree-cleanup
boundary to every running task, marks remaining applicable tasks `cancelled`,
writes the receipt, and returns `130`.

The human reporter serializes scheduler-owned start and completion lines while
task stdout and stderr remain in complete per-task logs. JSON mode emits no
progress text, so stdout contains only the final machine document. Completion
order never changes plan-ordered task evidence in the receipt.

Overall process exits are `0` for a passing graph, `1` for task failure or
blocking, `2` for CLI or manifest errors, and `3` for internal runner failures.
User cancellation returns conventional exit `130`.

## CI reporting and evidence

The GitHub reporter is selected explicitly with `--format github`; environment
detection never changes output contracts. It waits for execution and receipt
persistence, then emits task groups in deterministic plan order. Groups never
nest or overlap. Every task-derived context line has a non-command prefix so a
child process cannot inject a GitHub workflow command through its log output.

Failed, blocked, and cancelled results produce at most ten error annotations per
run. Each encoded annotation message is capped at 4 KiB and its title at 256
bytes. Failure context reads
at most the final 16 KiB and 50 complete lines of a task log without loading the
complete file into memory. Complete logs remain in the evidence directory.
Reporting does not reinterpret results: qualification failure still exits `1`,
configuration errors `2`, internal errors `3`, and interruption `130`.

Hosted lanes select explicit runner-temporary log and receipt paths. An
immutable, SHA-pinned artifact action runs under `always()` and treats missing
evidence as an error, so a normal task failure retains the same complete receipt
and logs as a passing run.

The setup-zcheck action contract requires an exact version without a leading
`v` and a caller-reviewed SHA-256 for the selected archive. It supports glibc
Linux x86-64 and ARM64, macOS x86-64 and Apple Silicon, and Windows x86-64. The
installer caps archive and expanded binary size, extracts exactly one regular
binary, selects `python3` on Unix and `python` on Windows, verifies
`zcheck --version`, and adds only its directory to `PATH`.
There is no latest-version resolution, source fallback, cache, task execution,
or publication in this repository. Version `0.0.1` is crates-first; binary
archives and action publication are outside that release boundary.

## Repository state

zcheck enforces repository-state policy. `preserve` fingerprints HEAD,
staged and unstaged binary diffs, untracked paths and contents, symlink targets,
and recursive submodule state. Comparing porcelain status text is insufficient
because a task can alter a file that was already dirty without changing its
status code. `clean` rejects initial dirt before starting tasks. `ignore` does
not invoke Git. Mutation diagnostics compare per-path index and worktree
fingerprints before reporting a tracked path, so unchanged pre-existing dirt
cannot displace the path that actually changed.

Logs and default receipts live outside the checkout. On Unix, each run
directory and task log is created with private permissions. Log filenames use a
bounded plan index and task-name prefix so every schema-valid name remains
portable. Schema-1 receipts record runner and manifest identity, component Git
fingerprints, host projection, selected roots, task evidence, and artifact
locations. Explicit environment-variable names are recorded, never their
values. Explicit `--logs-dir` and `--receipt` targets are create-new and must not
already exist. Repository-local targets are intentional runner-owned artifact
exceptions and are not treated as task mutations.

## zrail boundary

zcheck may execute `zrail check`. zrail statically reviews `zcheck.toml`, local
programs named by tasks, and explicit task inputs. `zcheck-core` never depends on
zrail, so the relationship has no library dependency cycle. The public
`zcheck-core` analysis surface and its external-consumer proof provide the
boundary for first-class static gate integrations.
