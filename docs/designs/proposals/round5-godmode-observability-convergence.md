---
title: trace-core as godmode's observability event shape
slug: godmode-observability-convergence
round: 5
status: draft
viability: medium
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# trace-core as godmode's observability event shape

## Problem

godmode's observability-as-infrastructure records every helper invocation/branching decision/lifecycle transition as JSONL; trace-core does the same via Step/Branch — two unrelated schemas for the same idea.

## Approach

godmode's observability infra constructs real Step/Branch values as it runs, instead of trace:: consuming godmode's JSONL after the fact via an adapter.

## API sketch

Proposed `impl From<GodmodeHelperEvent> for Step` — see verification note; this mapping is not as clean as a single From impl.

## Integration

Would make TraceGraph, trace-narrate, confidence-calibration, and trace-archive usable on godmode's own helper/subagent history for free.

## Verification notes

Confirmed godmode/skills/observability-as-infrastructure/SKILL.md is real, with event kinds skill.start/skill.complete/skill.error/decision/agent.start/agent.complete/agent.blocked, writing to .ctx/godmode/traces/trace.jsonl. But godmode's schema is flat and session/helper-scoped, not structurally identical to Step's typed StepOutcome/confidence/duration/branches shape — no clean 1:1 mapping exists (e.g. agent.blocked doesn't map onto any current StepOutcome variant).

## Notes

Downgraded from 'mostly just noticing they're the same' to real adapter design work — decide the lossy-mapping cases deliberately (esp. decision and agent.blocked events) before implementing.

## Prior art
No dedicated research agent was run for this one — this is a schema-mapping question between two
local, already-owned, already-verified systems (trace-core's Step/Branch and godmode's real
observability-as-infrastructure JSONL schema, both read directly — see Verification notes). The
open question (how to handle event kinds like agent.blocked that don't map cleanly onto any
current StepOutcome variant) is answered by deciding the mapping deliberately, not by external
research — there's no published standard for reconciling two bespoke internal event schemas.
