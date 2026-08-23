---
name: run-tracers
description: Build, run, test, and smoke-test the tracers (trace-lang) Rust workspace — run the end-to-end smoke driver, run the test suite, verify the four library crates work as a downstream consumer would.
---

# Run tracers (trace-lang)

This is a pure **library workspace** — four publishable crates
(`trace-lang-core`, `-task`, `-agent`, `-runtime`) plus a test-only crate
and an `xtask`. There is no binary or GUI; "running" it means driving the
public API. A committed smoke driver does exactly that.

All paths below are relative to the repo root.

## Prerequisites

Stable Rust toolchain (edition 2024) and `cargo-nextest`. Nothing else —
verified on macOS with plain `cargo`.

## Run (agent path): the smoke driver

A standalone cargo package (deliberately **not** a workspace member — it has
its own `[workspace]` table) that imports all four crates via path deps and
exercises one real flow end-to-end: Trace provenance → `spawn` → `join_all`
→ `speculate` → `run_with_escalation` → `TaskRegistry` checkpoint round-trip
through `FileCheckpointStore`.

```bash
cd .claude/skills/run-tracers/smoke && cargo run --quiet
```

Expected output (exits non-zero on any failure):

```
core: Trace ok — 1 step(s)
agent: spawn ok — "hello, world! (from Alice)"
runtime: join_all ok — 3 results
runtime: speculate ok — winner: "hello, race! (from Bold)"
runtime: run_with_escalation ok
task: checkpoint round-trip ok — <tmpdir>/tracers-smoke/registry.json
SMOKE OK: all 7 checks passed
```

Driver source: `.claude/skills/run-tracers/smoke/src/main.rs`. It doubles as
the workspace's only end-to-end usage example — extend it when new public
API lands.

## Build / check

```bash
cargo check --workspace
```

## Test

```bash
cargo nextest run --workspace     # 91 tests, ~all pass in <1s
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

## Gotchas

- **`cargo xtask` is not installed globally** — use
  `cargo run -p xtask -- <task>`. Available tasks: `fmt, fmt-check, lint,
  test, ci, pre-commit, pre-push`.
- **`xtask ci` is broken**: xtask delegates to the external `taskit` binary,
  which has no `ci` subcommand (`error: unrecognized subcommand 'ci'`). Run
  fmt-check/clippy/nextest directly instead (commands above).
- **`TraceErr` has no directly-constructible variants** from outside the
  crate — use the constructors `TraceErr::other(msg)`,
  `TraceErr::rejected(reason)`, `TraceErr::tool_failed(tool, msg)`.
- **Standalone packages under the repo need `[workspace]`** in their
  Cargo.toml or cargo tries (and fails) to treat them as workspace members.
- **`AgentRegistry::register` takes `Arc<dyn Agent<Input=I, Output=O>>`** —
  coerce explicitly (`Arc::new(MyAgent) as Arc<dyn Agent<...>>`) when the
  agent type is concrete.
- Speculate's winner is picked by **step confidence**, first candidate wins
  ties (guaranteed by test `ties_keep_first_candidate_in_order`).

## Release (for reference — verified 2026-08-23 cutting v0.2.2)

Branch flow is develop → staging → release → main, driven by `taskit`:

```bash
# bump 0.X.Y in Cargo.toml + crates/{runtime,task,agent}/Cargo.toml
# (inter-crate deps pin exact versions), commit on develop, then:
taskit flow auto          # promotes through all stages with CI gate, pushes
git tag vX.Y.Z main && git push origin vX.Y.Z
# publish from main, dependency order: core → agent → task → runtime
# CARGO_REGISTRY_TOKEN: op read 'op://cli/jkootvt72uguincgh7zgpkghz4/token'
```

`trace-lang-test` is not published (path deps carry no version fields).
