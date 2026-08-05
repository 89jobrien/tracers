---
title: Traces as the substrate for godmode:agent-improvement-loop
slug: agent-improvement-loop-substrate
round: 5
status: draft
viability: high
depends_on:
- trace-archive
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Traces as the substrate for godmode:agent-improvement-loop

## Problem

agent-improvement-loop's own first documented stage is 'collect traces' — presumably ad hoc log scraping today, not the structured Trace<T> this project exists to produce.

## Approach

Stage 1 becomes a direct TraceArchive query (agent/before/etc.) instead of log reconstruction; later stages (feedback, eval generation, HALO diagnosis) operate on causal_chain()/rejected_branches()/low_confidence() directly.

## API sketch

`archive.query().agent("Coder").before(last_week).run().await` — no new trace:: functionality, reuse of trace-archive's existing query builder.

## Integration

Entirely about making TraceArchive and existing Trace<T> introspection methods the literal data source for an existing skill's stage one.

## Verification notes

Confirmed godmode:agent-improvement-loop exists as a real skill whose own description starts with 'collect traces' as stage one.

## Dependencies

- trace-archive

## Notes

Most direct match of any proposal across all rounds — the target skill's own documented first step is exactly what trace:: already produces in structured form. Very little design work required once trace-archive ships.

## Prior art
No dedicated research agent was run for this one — the target skill (godmode:agent-improvement-loop)
was already confirmed real and its "collect traces" first stage confirmed to match this proposal's
premise directly (see Verification notes). This is the most directly-matched proposal across all
rounds; there's no external research question left to answer beyond what's already established.
