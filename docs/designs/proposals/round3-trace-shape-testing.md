---
title: Trace-shape testing
slug: trace-shape-testing
round: 3
status: draft
viability: high
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Trace-shape testing

## Problem

Testing an agent today only asserts on final output — an agent can get the right answer for the wrong reason and no test catches it.

## Approach

`assert_trace!` macro/builder over Trace::causal_chain() with contains_step/confidence_below/escalates_to/never_step assertions.

## API sketch

`#[trace_test]` attribute; `assert_trace!(outcome.trace, { contains_step("search"); confidence_below("search", 0.5); escalates_to("HumanReviewer"); never_step("publish"); })`

## Integration

New trace-test dev-dependency crate depending on tracers-core/tracers-agent. Runs against SpawnOutcome<O> and RunOutcome<O>.

## Verification notes

Confirmed SpawnOutcome{trace,context,escalation} and RunOutcome{trace,context,unresolved} are real (crates/agent/src/spawn.rs, crates/runtime/src/execute.rs), and AgentContext.delegation_chain is a real pub Vec<String> — escalates_to() is a direct field check, not speculative.

## Notes

Cheapest, most self-contained proposal in round three. Ship early.

## Prior art

- **QuickCheck** (Claessen &amp; Hughes, ICFP 2000, https://dl.acm.org/doi/pdf/10.1145/636517.636527) — the foundational property-based-testing paper. This proposal's `assert_trace!` is conceptually much closer to a property-based-testing assertion (an invariant checked over the space of possible execution traces) than to classical example-based testing. Its stateful/model-based-testing extension (carried forward into Hypothesis's `RuleBasedStateMachine` and proptest's `proptest-state-machine`) generates *sequences of operations* and checks invariants over the resulting execution trace against a reference model — the direct academic precedent for asserting shape over a trace rather than only a final value.
- **A Trace-Based Assurance Framework for Agentic AI Orchestration** (Paduraru, Bouruc, Stefanescu, arXiv:2603.18096, 2026) — strong independent corroboration: this paper proposes formal contracts validated against execution traces (verifying escalation fires when confidence thresholds are breached, correct step sequencing, governance compliance across the whole path) — essentially this exact proposal, published independently. This is real evidence the design direction is sound and not idiosyncratic to this project.
- **"When Tools Fail: Benchmarking Dynamic Replanning and Anomaly Recovery in LLM Agents"** (arXiv:2606.05806, 2026) — benchmarks whether agents replan/recover correctly under tool failure, directly relevant to what escalates_to()/never_step() assertions would be checking in practice.

**Bottom line**: this is now the best-externally-corroborated proposal across all rounds — an independent 2026 paper describes essentially the same mechanism, and the underlying testing philosophy (property-based testing over execution traces) is a mature, 25-year-old discipline, not a novel idea. No significant risk surfaced by this research.
