# Contributing

Thanks for your interest in `trace-lang`. This is an early-stage design + reference
implementation — expect the language surface to be more stable as a discussion
artifact than as running code, and the Rust crates (`trace-lang-core`, `trace-lang-task`,
`trace-lang-agent`, `trace-lang-runtime`) to be the part that actually compiles and runs
today.

## Before you start

For anything beyond a small fix, open an issue first to discuss the approach —
especially for changes to `Trace<T>`, `Task`, `Agent`, or any other core type, since
those are meant to stay stable across the whole workspace.

## Development

```bash
cargo check --workspace          # fast feedback
cargo test --workspace           # run all tests, including doctests
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
```

All four must pass clean before opening a PR. `cargo fmt --all` will fix formatting
issues in place.

### Conventions

- Hexagonal architecture: keep domain logic in `trace-lang-core` and `trace-lang-task` free
  of any runtime/IO concern — I/O goes behind a port trait (see `CheckpointStore`).
- `trace-lang-core` must not depend on `trace-lang-task` — the dependency graph flows one
  way: `core -> task`, `core -> agent -> runtime`.
- Builder pattern for public types (`Task::with_priority()`, `Step::with_confidence()`).
- No `unwrap()` in library code — use `?` or explicit error handling. `unwrap()` in
  tests is fine but should carry `.expect("why this can't fail")`.
- Prefer editing existing files over adding new abstractions; keep changes scoped to
  what the fix or feature actually needs.

### Testing

New code should be covered at whichever level fits the change:

- **Unit tests** for any new function — always, first.
- **Property tests** (`proptest`) when an invariant should hold across many inputs
  (see `crates/agent/src/context.rs` for an example).
- **Model-check proofs** (`cargo kani`) for arithmetic or state-machine logic where
  you want to prove an invariant holds for *all* reachable inputs, not just sampled
  ones (see `crates/runtime/src/speculate.rs`'s `first_max_index` proofs).
- **Conformance tests** for any new implementation of a port trait like
  `CheckpointStore` — reuse `assert_checkpoint_store_contract`, don't write a new
  ad-hoc test.
- **Integration tests** (`tests/` directory) when the change is about wiring two
  crates together, not the logic within one.
- **Regression tests** for every bug fix — reproduce it with a minimal test before
  fixing it.

### Documentation

Doc comments on public items should explain the *why*, not restate the *what* —
a non-obvious constraint, an invariant a caller must not violate, or the reason
behind a design choice. Skip the comment if the name and signature already say it.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`,
types `feat`/`fix`/`refactor`/`test`/`docs`/`chore`/`ci`, scope the crate or module
name (e.g. `runtime`, `task`).

## Reporting issues

Open a GitHub issue. For bugs, include a minimal repro if possible — a failing test
is even better than a description.
