---
title: "speculate_race \u2014 early-exit speculative branching"
slug: speculate-race
round: 1
status: draft
viability: medium
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# speculate_race — early-exit speculative branching

## Problem

speculate() runs every candidate to completion before picking a winner — wasteful when one candidate clears an acceptable bar quickly.

## Approach

Race candidates via FuturesUnordered + tokio::select!; cancel the rest once one crosses `threshold`; fall back to speculate()'s highest-confidence-wins if none do.

## API sketch

`async fn speculate_race<I, O>(candidates: Vec<(String, Arc<dyn Agent<Input=I,Output=O>>)>, input: I, threshold: f64) -> Trace<O>`; requires new `BranchOutcome::Cancelled` variant alongside Taken/Rejected.

## Integration

Additive sibling to speculate() in tracers-runtime — same join_all-adjacent pattern, confirmed real and tested in crates/runtime/src/speculate.rs.

## Verification notes

speculate()'s existing confidence-fold logic and tie-breaking behavior confirmed via direct read and its passing test suite (ties_keep_first_candidate_in_order).

## Notes

BranchOutcome is a public enum used elsewhere (Branch, Trace::all_branches) — audit all match sites before adding Cancelled.

## Prior art
No dedicated research agent was run for this one. The "race N futures, cancel the rest once one
crosses a threshold" pattern is a standard concurrent-programming idiom (`FuturesUnordered` +
`select!` in Rust, or `Promise.any`-with-early-exit in other ecosystems) — not something requiring
academic grounding. The circuit-breaker research done for failure-learning-decay-reference is
tangentially relevant (Envoy/resilience4j also race against thresholds) but doesn't add anything
beyond what's already in that doc. The real open question — whether cancelled candidates need a
third BranchOutcome::Cancelled variant — is an internal API-design decision, not a research one.
