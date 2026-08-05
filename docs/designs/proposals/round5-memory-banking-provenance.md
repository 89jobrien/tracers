---
title: Provenance-backed godmode:memory-banking
slug: memory-banking-provenance
round: 5
status: draft
viability: medium
depends_on:
- trace-archive
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Provenance-backed godmode:memory-banking

## Problem

godmode's memory-bank entries are prose summaries with no link back to the trace (if any) that established a given fact — a wrong entry has no traceable origin.

## Approach

`MemoryBankEntry { fact, source: Option<TraceRef>, confidence: Option<f64> }` — additive, optional provenance, not a requirement for every entry.

## API sketch

`struct MemoryBankEntry { fact: String, source: Option<TraceRef>, confidence: Option<f64> }`

## Integration

TraceArchive is the natural backing store for what source: Some(TraceRef) points into.

## Verification notes

Confirmed godmode:memory-banking exists as a real skill (.ctx/memory-bank/, prompt injection via lifecycle hooks).

## Dependencies

- trace-archive

## Notes

Most memory-bank entries will continue to come from ordinary LLM summarization, not trace:: runs specifically — a mostly-None source field may feel like dead weight; the value is concentrated in the trace::-originated subset.

## Prior art
No dedicated research agent was run for this one — this is an internal-integration proposal
between two local, already-owned systems (a proposed TraceArchive and the real godmode
memory-banking skill). Provenance-linking a summarized fact back to its source is conceptually
the same idea as the W3C PROV model researched for trace-graph (round 1) — see that doc's Prior
art section — but this proposal doesn't add anything beyond what's already covered there.
