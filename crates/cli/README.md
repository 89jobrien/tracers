# trace-cli

[![crates.io](https://img.shields.io/crates/v/trace-cli.svg)](https://crates.io/crates/trace-cli)
[![docs.rs](https://docs.rs/trace-cli/badge.svg)](https://docs.rs/trace-cli)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://github.com/89jobrien/trace-lang)

Inspect `trace::` checkpoint files from the command line. Installs a single
binary, `trace`.

A `TaskRegistry` checkpoint is a complete, serialized picture of a pipeline:
every task, its status, its dependency edges, and the `TraceRef` linking each
terminal state back to the execution that produced it. `trace` reads that file
and answers the questions you actually have about it.

## Install

```bash
cargo install --path crates/cli
# or, from a checkout
cargo run -p trace-cli -- list ./checkpoint.trace.json
```

## Commands

| command | question it answers |
| --- | --- |
| `trace list <checkpoint> [--status <s>]` | what is in this pipeline, and what state is it in? |
| `trace show <checkpoint> <task>` | everything about one task, with its dependencies named |
| `trace chain <checkpoint> <trace>` | which task produced this trace, and what fed into it? |
| `trace diff <before> <after>` | what moved between these two checkpoints? |

Every command takes `--json`. An agent-first CLI whose output only a human can
read is not agent-first.

### `list`

```text
$ trace list checkpoint.trace.json
id        status    priority  title
6d0fae38  done      high      survey the schema
f32d9148  paused    normal    design the migration
72440257  pending   normal    write the migration
98bf9fdd  pending   low       update the changelog

4 task(s)
```

`--status pending|running|done|failed|paused` filters. Output is sorted by
priority, then title, then id — `TaskRegistry` is a `HashMap`, so without that
tie-break the order would change between runs and the output would be
impossible to diff.

### `show`

Takes a full task id or any unambiguous prefix of one. An ambiguous prefix is
an error, not a lucky first match.

```text
$ trace show checkpoint.trace.json f32d9148
id            f32d9148-b656-4831-b051-1e2c4fb83a59
title         design the migration
status        paused
priority      normal
assigned to   Designer
trace         trace::ad107b0f-b843-436f-a919-4ec5434507db
waiting on    approve the destructive migration?
context       {"drops_columns":2}
depends on
  6d0fae38  survey the schema [done]
```

### `chain`

Accepts `trace::<uuid>`, a bare uuid, or a prefix.

```text
$ trace chain checkpoint.trace.json ad107b0f
trace::ad107b0f-b843-436f-a919-4ec5434507db
  produced by  design the migration [paused]

upstream, nearest first
  6d0fae38  done      survey the schema   trace::e7569a63-...
```

Tasks point at traces, not the other way round, so finding the task behind a
trace is a scan of the checkpoint. That is fine at checkpoint scale and honest
about what the data model stores.

### `diff`

```text
$ trace diff before.trace.json after.trace.json
+ 1f0c8d2a  backfill
~ f32d9148  design the migration: paused → pending
```

Compares status *labels*, so a task that failed twice with different error
messages is not reported as a change.

## Design

Checkpoints are read through `trace_lang_task::CheckpointStore`, never
`std::fs` directly — pointing `trace` at another backing store is a matter of
swapping the adapter, per the workspace's hexagonal rule.

Errors surface as `TraceErr`, which derives `miette::Diagnostic`, so a missing
file or an unknown id prints a code and a help line rather than a `Debug` dump,
and exits non-zero.

The query functions (`list`, `resolve_task`, `chain`, `diff`) are public library
API, so anything that wants these answers in-process can call them without
shelling out.

## Testing

```bash
cargo test -p trace-cli
```

Unit tests cover the queries against in-memory registries; `tests/` runs the
real binary against real checkpoint files, covering argument parsing, exit
codes, and what actually lands on stdout.
