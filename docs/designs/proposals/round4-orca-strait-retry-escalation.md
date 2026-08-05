---
title: orca-strait-shaped retry escalation
slug: orca-strait-retry-escalation
round: 4
status: draft
viability: low
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# orca-strait-shaped retry escalation

## Problem

EscalationAction has Delegate (hand off to a different agent) but nothing meaning 'retry this same agent, per a specific strategy.'

## Approach

New `EscalationAction::Retry(RetryStrategy)` variant with Immediate/Backoff/NarrowedScope strategies.

## API sketch

`enum RetryStrategy { Immediate { max_attempts: usize }, Backoff { max_attempts: usize, base_delay: Duration }, NarrowedScope { max_attempts: usize } }`

## Integration

run_with_escalation would need a Retry branch alongside its existing Delegate handling.

## Verification notes

FALSIFIED: the original proposal's central claim — that orca-strait's own documentation names a currently-unimplemented RETRY_STRATEGY.md gap — does not check out. A full search of ~/dev/orca-strait (all files, case-insensitive 'retry' grep) found no such file and no documented gap matching this description.

## Notes

Downgraded from 'standout of round four' to ordinary speculative-feature tier. The retry-vs-delegate distinction may still be worth adding on its own merits, but not for the reason originally given. See crs-rewrite-retry-strategy (round 6) for a real, verified working mechanism (coursers' crs rewrite) that could ground this instead.

## Prior art
No research agent was run for this one beyond the verification already performed, which is the
important finding here: the proposal's central motivating claim (a documented RETRY_STRATEGY.md
gap in orca-strait) was checked directly against the actual orca-strait repository and found to
be false — no such file or documented gap exists (see Verification notes). External research into
retry-strategy design generally wouldn't rescue a proposal whose own stated justification doesn't
hold; if this feature is still wanted, it needs a real justification first, not more literature.
