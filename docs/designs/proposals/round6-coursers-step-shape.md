---
title: trace-core as coursers' Step/Branch event shape
slug: coursers-step-shape
round: 6
status: draft
viability: high
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# trace-core as coursers' Step/Branch event shape

## Problem

coursers pre/post decisions are structurally a rejected Branch and a Step with Taken/Failed outcomes, but serialized as coursers' own {decision, reason} JSON shape.

## Approach

`impl From<CoursersDecision> for Step` — a Bash-capable agent pushes one Step per coursers pre invocation, using step.rejected(...) for denials.

## API sketch

`impl From<CoursersDecision> for Step { fn from(decision: CoursersDecision) -> Self { ... } }`

## Integration

Same convergence pattern as godmode-observability-convergence, but scoped to one simple, already-narrow event type (Bash tool calls), making it a much smaller first step.

## Verification notes

Confirmed coursers' rule schema (id/pattern/exceptions/message) is real, matching README exactly.

## Notes

Decide whether every Bash call needs a Step or only ones coursers actually intercepted, to avoid trace noise from successful, unblocked commands.

## Prior art
No dedicated research agent was run for this one — this is a schema-mapping question between two
local, already-owned, already-verified systems (trace-core's Step and coursers' real, verified
decision JSON shape — see Verification notes). No external research question exists here.
