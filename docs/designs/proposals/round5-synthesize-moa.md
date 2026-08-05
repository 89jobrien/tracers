---
title: "synthesize() \u2014 a godmode:moa-shaped combining strategy"
slug: synthesize-moa
round: 5
status: draft
viability: medium-high
depends_on:
- converge
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# synthesize() — a godmode:moa-shaped combining strategy

## Problem

trace-runtime has 'pick one' (speculate) and 'agree or don't' (converge); nothing represents 'blend multiple outputs into one' the way godmode:moa does.

## Approach

Run proposers concurrently (same join_all pattern), then feed all outputs into a dedicated synthesizer Agent whose own reasoning is captured as a real Trace<S>.

## API sketch

`async fn synthesize<I, O, S>(proposers: Vec<(String, Arc<dyn Agent<Input=I,Output=O>>)>, input: I, synthesizer: &dyn Agent<Input=Vec<(String,O)>,Output=S>) -> Trace<S>`

## Integration

Completes the combinator family tracers-runtime already started: speculate (pick one), converge (agree or don't), synthesize (blend).

## Verification notes

Confirmed godmode:moa exists as a real skill; confirmed the join_all fan-out mechanics this reuses are real and tested.

## Dependencies

- converge

## Notes

Open question worth resolving early: does the synthesizer need each proposer's full Trace<O> (richer reasoning context) or just their final O values (simpler Input type)? Affects the function's core signature.

## Prior art

- **"Mixture-of-Agents Enhances Large Language Model Capabilities"** (Wang, Wang, Athiwaratkun, Zhang, Zou, arXiv:2406.04692, ICLR 2025) — the original MoA paper this proposal is directly named after. Layered proposer-then-synthesize architecture, reports 65.1% on AlpacaEval 2.0 vs. GPT-4o's 57.5% using only open models. Important caveat: **this compares against a single strong model, not against majority-vote/best-of-N under matched compute** — the paper does not establish that synthesis beats voting.
- **"Rethinking Mixture-of-Agents: Is Mixing Different Large Language Models Beneficial?"** (Li, Lin, Xia, Jin, Princeton, arXiv:2502.00674, 2025) — the actual head-to-head comparison, and it's a real negative finding worth taking seriously: **Self-MoA** (repeated sampling from a single top model, no mixing) *beats* standard cross-model MoA in most cases tested (+6.6% AlpacaEval 2.0, +3.8% average across MMLU/CRUX/MATH). Root cause: MoA's output quality is highly sensitive to the *average* quality of the pooled proposers — mixing in a weaker model drags the synthesized output down. Heterogeneous proposers are a liability when quality varies, not an automatic asset.
- **"Beyond Majority Voting: LLM Aggregation by Leveraging Higher-Order Information"** (arXiv:2510.01499) — a middle-ground alternative: statistical weighted-aggregation methods (Optimal Weight, Inverse Surprising Popularity) that provably beat plain majority voting without needing full LLM synthesis — worth considering as a cheaper alternative to a dedicated synthesizer agent for some use cases.
- Latency/cost: layered aggregation means no output until the final synthesis pass completes, and proposer-call-count × aggregator-call-count multiplies inference cost versus a single model or a simple vote — a real, established tradeoff, not hypothetical.

**This is a genuine risk finding, not just confirmation**: the strongest direct evidence found argues *against* assuming synthesis is a free upgrade over converge()'s majority-vote approach. Before implementing, this design doc should explicitly flag: (1) proposer-quality sensitivity — a weak proposer in the pool can drag down the synthesized result, unlike a straightforward best-of-N or majority vote which is more robust to one bad candidate; (2) the cost/latency multiplier is real and unavoidable in the proposed shape. Consider whether `synthesize()` should default to same-model repeated sampling (mirroring Self-MoA's finding) rather than assuming heterogeneous proposers are inherently better, and note this explicitly as an open question alongside the existing Trace<O>-vs-O signature question.
