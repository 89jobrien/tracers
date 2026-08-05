---
title: "converge() \u2014 consensus across independent agents"
slug: converge
round: 3
status: draft
viability: high
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# converge() — consensus across independent agents

## Problem

speculate() picks a best-of-N winner; nothing measures agreement when running the *same* agent multiple times on the same input.

## Approach

Run the same agent N times concurrently, tally identical outputs, return majority value's trace plus the full agreement breakdown.

## API sketch

`struct ConsensusResult<O: PartialEq> { agreement: HashMap<O, usize>, majority: Option<O>, unanimous: bool }`; `async fn converge<I, O>(agent: &dyn Agent<Input=I,Output=O>, input: I, runs: usize) -> (Trace<O>, ConsensusResult<O>) where O: Eq + Hash`

## Integration

Same futures::future::join_all pattern already shipped in tracers-runtime's join_all and speculate — confirmed real in crates/runtime/src/join.rs and speculate.rs.

## Verification notes

Confirmed the concurrent-fan-out mechanics this depends on are real, tested, and directly reusable.

## Notes

Needs Eq + Hash on O — free-text outputs rarely produce byte-identical results even from consistent judgment; may need a semantic-similarity variant for text outputs eventually, out of scope for v1.

## Prior art

- **Self-Consistency Improves Chain of Thought Reasoning in Language Models** (Wang, Wei, Schuurmans, Le, Chi, Narang, Chowdhery, Zhou, arXiv:2203.11171, 2022) — the foundational paper this proposal's core mechanism is based on: sample multiple times, take majority vote. Real, well-established backing for the basic idea.
- **SelfCheckGPT: Zero-Resource Black-Box Hallucination Detection** (Manakul, Liusie, Gales, arXiv:2303.08896, 2023) — uses divergence across repeated samples as a hallucination signal with no external ground truth needed, corroborating agreement-as-signal without waiting for delayed ground truth.
- **"When LLMs Agree, Are They Right? Auditing Self-Consistency and Cross-Model Agreement as Confidence Signals"** (arXiv:2607.08065, 2026) — the important caveat: this is an explicit 2026 audit of whether agreement is *always* trustworthy as a confidence signal, rather than assuming it. Worth reading closely before finalizing `converge()`'s semantics — agreement and correctness can decouple under systematic model biases shared across all N runs (e.g. if the same underlying model produces the same wrong answer consistently, unanimous agreement doesn't mean correct).

**Practical implication for this design**: `unanimous: bool` alone risks being read as a strong correctness signal when the 2026 audit literature says that's not always warranted, especially since all N runs here use the *same* agent/model (unlike cross-model agreement, which the audit paper treats as a stronger signal). Consider documenting this caveat directly in `ConsensusResult`'s doc comment rather than letting callers assume unanimity implies correctness.
