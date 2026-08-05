---
title: TraceArchive::query() in place of crs discover
slug: trace-archive-vs-crs-discover
round: 6
status: draft
viability: medium-high
depends_on:
- trace-archive
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# TraceArchive::query() in place of crs discover

## Problem

crs discover scans raw Claude Code session JSONL for near-misses — real, useful, but built on regex-parsing unstructured transcripts after the fact.

## Approach

Where a session was trace::-instrumented, crs discover's job becomes a structured TraceArchive query instead of a regex pass; stays additive, not a replacement, since crs discover's value is partly that it works on any session, instrumented or not.

## API sketch

`archive.query().agent_name_contains("Bash").matching_rule_not_applied().run().await` — extends TraceArchive's existing query() builder with a coursers-specific predicate.

## Integration

Same 'consuming tool gets a structured source instead of reconstructing from logs' pattern as whatidid-event-stream and agent-improvement-loop-substrate.

## Verification notes

CONFIRMED via direct file inspection: crs discover is real, with source, integration tests (discover_integration.rs), and even a design doc (docs/superpowers/specs/2026-04-06-crs-discover-design.md).

## Dependencies

- trace-archive

## Notes

Decide whether 'matching a rule but not applied' becomes a first-class TraceArchive query predicate or stays a coursers-side adapter layered on a more generic archive — leaning toward the latter to keep TraceArchive itself tool-agnostic.

## Prior art
No dedicated research agent was run for this one — this is an internal-integration proposal
between two local, already-owned, already-verified systems (a proposed TraceArchive and coursers'
real crs discover — see Verification notes, which confirms real source, tests, and a design doc
for crs discover). No external research question exists.
