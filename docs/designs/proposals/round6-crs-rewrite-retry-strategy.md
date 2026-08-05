---
title: crs rewrite as EscalationAction::Retry(RetryStrategy::Rewritten)
slug: crs-rewrite-retry-strategy
round: 6
status: draft
viability: medium
depends_on:
- orca-strait-retry-escalation
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# crs rewrite as EscalationAction::Retry(RetryStrategy::Rewritten)

## Problem

orca-strait-retry-escalation proposed Retry(RetryStrategy) as a new lifecycle hook outcome; coursers' crs rewrite already does exactly this for Bash commands today.

## Approach

Add `RetryStrategy::Rewritten { pattern, replace, max_attempts }`, matching crs rewrite's real [[rewrites]] TOML rule format, so rewrites become visible in a causal chain instead of transparent to the agent.

## API sketch

`enum RetryStrategy { .., Rewritten { pattern: String, replace: String, max_attempts: usize } }`

## Integration

Worked instance of EscalationAction::Retry using coursers' actual, verified rewrite mechanism as the concrete shape.

## Verification notes

CONFIRMED via direct file inspection: crs rewrite is real, with actual Rust source (crates/core/src/hook/rewrite.rs), integration tests (rewrite_integration.rs, rewrite_binary.rs). This is a materially stronger foundation than orca-strait-retry-escalation's own (debunked) motivating claim.

## Dependencies

- orca-strait-retry-escalation

## Notes

Note the asymmetry with its dependency: orca-strait-retry-escalation's original justification (a documented gap) was falsified, but crs-rewrite itself is real and working — this proposal's own foundation stands independent of that debunked claim. Generalizing Rewritten beyond Bash commands needs Input to support pattern-based rewriting generically — may stay Bash/coursers-specific for v1.

## Prior art
No dedicated research agent was run for this one beyond the verification already performed, which
matters: unlike its dependency (orca-strait-retry-escalation, whose motivating claim was falsified),
this proposal's own foundation (crs rewrite) was independently confirmed real via direct file
inspection (crates/core/src/hook/rewrite.rs, integration tests) — see Verification notes. The
open question (does a rewritten retry consume the same step budget as the original attempt) is a
local design decision.
