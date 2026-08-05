---
title: Trace archive (doob-backed)
slug: trace-archive
round: 2
status: draft
viability: high
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Trace archive (doob-backed)

## Problem

TaskRegistry::save/load gives crash-resumability within one session, not a queryable cross-session, cross-project trace history.

## Approach

New `trace-archive` crate persisting completed traces into doob's existing storage (SurrealDB/RocksDB) alongside todos, not a parallel database.

## API sketch

`struct TraceArchive { store: doob::Store }`; `impl TraceArchive { async fn record<T: Serialize>(&self, trace: &Trace<T>, task: &Task) -> Result<(), TraceErr>; fn query(&self) -> ArchiveQuery }`; `impl ArchiveQuery { fn goal_contains(self, text: &str) -> Self; fn before(self, when: DateTime<Utc>) -> Self; fn agent(self, name: &str) -> Self; async fn run(self) -> Vec<ArchivedTrace> }`

## Integration

doob already has a working CheckpointStore adapter — confirmed real and tested in doob-core/src/tracers_store.rs (DoobCheckpointStore, conformance-tested via assert_checkpoint_store_contract). TraceArchive can sit next to it using the same TodoRepository port.

## Verification notes

Confirmed real, working, tested integration code already exists — better-grounded than the original proposal claimed.

## Notes

Read doob-core's own risk note (docs/plans/2026-07-31-wire-tracers-task-registry.md) on the block_on-bridging risk before building on it.

## Prior art
No dedicated research agent was run for this one. Cross-session, queryable execution history is
a standard data-engineering pattern (audit logs, event sourcing, data warehousing) with no
trace::-specific research question — the actual design content is entirely about reusing an
already-verified local integration point (doob's CheckpointStore), not an open research problem.
