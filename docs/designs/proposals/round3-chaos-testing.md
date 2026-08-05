---
title: Chaos testing for agents
slug: chaos-testing
round: 3
status: draft
viability: high
depends_on:
- trace-shape-testing
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Chaos testing for agents

## Problem

on_step_failure/on_budget_exceeded/on_low_confidence are the code paths least likely to be exercised by normal test inputs.

## Approach

`ChaosAgent<A: Agent>` decorator wraps any agent, deliberately breaking budget/tool-call/confidence conditions before delegating to inner.run().

## API sketch

`struct ChaosPolicy { fail_tool_calls: f64, force_budget_exhaustion: bool, inject_low_confidence: Option<f64> }`; `struct ChaosAgent<A: Agent> { inner: A, policy: ChaosPolicy }` implementing Agent, delegating hooks unchanged to inner.

## Integration

Pairs directly with trace-shape-testing: wrap, force a condition, assert_trace! confirms the declared EscalationAction actually fired.

## Verification notes

Confirmed AgentContext's fields (budget, steps_taken, etc.) are all pub (crates/agent/src/context.rs) — ctx.budget = Some(0) in the sketch works exactly as written, no setter needed.

## Dependencies

- trace-shape-testing

## Notes

See fuzz-corpus-chaos-source (round 6) for a real, already-curated adversarial input source (coursers' fuzz corpus) instead of hand-picked probabilities.

## Prior art

- **Principles of Chaos Engineering** (https://principlesofchaos.org/) and the original **Netflix Chaos Monkey** (2011-2012, https://netflixtechblog.com/the-netflix-simian-army-16e57fbab116) — chaos-engineering doctrine is explicit and unambiguous: injected failures must "reflect real-world events," prioritized by actual impact or frequency. Chaos Monkey worked specifically because instance death was a measured, recurring production reality at Netflix, not an invented scenario. Judged against this standard, the sketch's hand-specified `ChaosPolicy` probabilities (`fail_tool_calls: 0.3`, etc.) are **doctrinally weaker** than sourcing failure modes from data the system actually produced.
- **ReliabilityBench** (arXiv:2601.06112, 2026) — "the first systematic application of chaos engineering principles to LLM agent evaluation." Notably, even this paper's fault probabilities are hand-specified (a fault taxonomy), not sourced from real incident logs — it explicitly approximates realism rather than replaying genuine failures, which is an honest concession worth noting rather than a fully-solved reference implementation.
- **"Agents of Chaos"** (arXiv:2602.20021, 2026) — a two-week live adversarial red-team of six autonomous agents found critical failure-handling gaps in 10 of 11 scenarios under *real* adversarial pressure, not synthetic fault injection — further evidence that realistic/adversarial inputs surface more genuine gaps than arbitrary probability distributions.
- **SIRAJ** (arXiv:2510.26037, 2026) — automated red-teaming via structured reasoning, an alternative failure-discovery method worth comparing against hand-tuned chaos probabilities.

**This directly validates and strengthens round six's fuzz-corpus-chaos-source proposal**: chaos-engineering's own doctrine argues for exactly what that proposal suggests — source injected failures from coursers' real fuzz corpus rather than (or in addition to) the hand-specified `ChaosPolicy` probabilities sketched here. Recommend treating `ChaosPolicy`'s probability fields as a fallback/supplement for failure classes not yet represented in a real corpus, not the primary mechanism.
