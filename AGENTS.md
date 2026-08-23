# Agent instructions

Read `README.md` and the owning module contract before editing code.

## Loop

1. Keep each change inside one coherent ticket-sized scope.
2. Keep every Rust source file at or below 300 lines.
3. Keep `lib.rs`, `main.rs`, and `mod.rs` declarative.
4. Put tests in sibling `*_test.rs` files or integration-test directories.
5. Run the task commands represented by `zcheck.toml` before finishing.
6. When `zrail.toml` or `zrail.lock` changes, run `zrail diff` and report every
   grant, debt increase, or unknown change.

Do not add a `scripts/check` wrapper. Simple commands belong in `zcheck.toml`;
scripts are reserved for substantive implementation logic.

Do not weaken a rail merely to make an implementation pass.

Never run `zrail update --accept-grants`, weaken `zrail.toml`, or replace
`zrail.lock` without explicit human authorization. Report the semantic diff
instead.
