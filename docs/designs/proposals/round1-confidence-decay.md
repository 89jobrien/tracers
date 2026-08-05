---
title: Confidence decay
slug: confidence-decay
round: 1
status: draft
viability: medium
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Confidence decay

## Problem

A step's confidence score is a snapshot; nothing distinguishes 'confident and current' from 'confident and stale.'

## Approach

Optional `DecayCurve { half_life }` attached per-step via `Step::with_decay()`; `Trace::confidence_at(at: DateTime<Utc>)` computes exponential decay from `started_at`. Steps with no decay curve never decay (default, additive).

## API sketch

`struct DecayCurve { half_life: Duration }`; `impl Step { fn with_decay(mut self, half_life: Duration) -> Self }`; `impl<T> Trace<T> { fn confidence_at(&self, at: DateTime<Utc>) -> f64; fn low_confidence_below_at(&self, threshold: f64, at: DateTime<Utc>) -> Vec<&Step> }`

## Integration

Step.confidence: Option<f64> and Step.started_at: DateTime<Utc> already exist (crates/core/src/step.rs) — this is mechanically a straightforward addition, same shape as low_confidence_below.

## Verification notes

Confirmed Step's real fields support this without new plumbing.

## Notes

See failure-learning-decay-reference (round 6) — coursers ships a real, tuned THRESHOLD-based decay model as an alternative reference implementation. Resolve whether threshold and exponential decay are two variants or whether one subsumes the other before implementing.

## Prior art

Industry precedent (Hystrix, resilience4j, Polly, Envoy outlier detection, AWS SDK adaptive retry — none arxiv) consistently composes a threshold/counting *trigger* with a separate exponential *penalty/recovery* curve, rather than treating them as competing single mechanisms — supports shipping both a threshold-based and an exponential DecayCurve variant, not picking one.

### Foundational (2022-2023) — establishes that self-reported LLM confidence is unreliable

- **Do Language Models Know When They Don't Know? / Language Models (Mostly) Know What They Know** (Kadavath et al., Anthropic, arXiv:2207.05221, 2022) — earliest large-scale study showing LLMs' internal probability estimates are reasonably calibrated in-distribution but degrade badly out-of-distribution and under free-form generation. Foundational motivation for any external calibration/decay layer.
- **Just Ask for Calibration** (Tian, Mitchell, Zhou, Sharma, Rafailov, Yao, Finn, Manning, EMNLP 2023, arXiv:2305.14975) — shows RLHF-tuned models' raw token probabilities are poorly calibrated, but verbalized confidence is closer to calibrated (~50% ECE reduction on TriviaQA/SciQ/TruthfulQA). Useful baseline for what "ask the model its confidence" alone gets you before any decay/calibration is applied.
- **Self-Consistency Improves Chain of Thought Reasoning in Language Models** (Wang, Wei, Schuurmans, Le, Chi, Narang, Chowdhery, Zhou, arXiv:2203.11171, 2022) — foundational self-consistency paper; agreement fraction across repeated samples is widely reused downstream as an implicit confidence signal, directly relevant to how converge() output could inform a decay/trust adjustment.
- **SelfCheckGPT: Zero-Resource Black-Box Hallucination Detection** (Manakul, Liusie, Gales, arXiv:2303.08896, 2023) — uses divergence across repeated stochastic samples as a hallucination (low-confidence) signal with no external ground truth needed — a candidate mechanism for feeding real-time signal into confidence_at() without waiting on delayed ground truth.
- **Confidence Calibration and Rationalization for LLMs via Multi-Agent Deliberation** (arXiv:2404.09127, 2024) — proposes Collaborative Calibration: multiple agents deliberate and group consensus recalibrates a single model's confidence post-hoc, training-free. Directly relevant as an external-signal source for confidence_at() adjustments, not just decay by elapsed time.

### 2025-2026 — agentic-trajectory-specific work

- **Trust Between AI Agents: Measuring Formation, Breakage, and Recovery** (arXiv:2606.14923, 2026) — empirically measures inter-agent trust via reduced verification behavior. Trust forms fast, breaks immediately on failure, and recovers *more slowly* than it forms. Critically: clustered failures sustain suspicion far longer than the same failure count spread over time — decay is not a clean function of elapsed time alone.
- **DynaTrust: Defending Multi-Agent Systems Against Sleeper Agents via Dynamic Trust Graphs** (arXiv:2603.15661, 2026) — models trust as a continuously-evolving graph rather than a scalar decaying score, explicitly to prevent an agent from accumulating credit via good behavior and then defecting (a static/slowly-decaying score is exploitable this way).
- **Agentic Confidence Calibration** (arXiv:2601.15778, 2026) — Holistic Trajectory Calibration (HTC): extracts features from an agent's *entire* multi-step trajectory rather than one output, closely analogous to computing confidence_at() over a whole Trace<T> rather than per-step in isolation.
- **The Confidence Dichotomy: Analyzing and Mitigating Miscalibration in Tool-Use Agents** (arXiv:2601.07264, 2026) — different tool types affect calibration differently; a single global decay curve may not fit every step kind (tool call vs. reasoning vs. delegation).
- **Confidence Laundering in Agent Systems: Why Uncertainty Needs a Latent Carrier** (arXiv:2606.20662, 2026) — argues raw confidence gets distorted as it passes between agent steps/sub-agents and should be carried as an explicit structured object through the pipeline — directly supports Trace<T>/Step as the right home for this data rather than an external tracker.
- **Uncertainty Quantification in LLM Agents: Foundations, Emerging Challenges, and Opportunities** (arXiv:2602.05073, 2026) — survey; states plainly that naive uncertainty estimates from an underlying LLM cannot be used directly as agent confidence, motivating an external calibration/decay layer rather than trusting self-reported scores as-is.
- **When LLMs Agree, Are They Right? Auditing Self-Consistency and Cross-Model Agreement as Confidence Signals** (arXiv:2607.08065, 2026) — audits whether agreement across repeated/cross-model samples is a *trustworthy* confidence signal, rather than assuming it — relevant caveat if converge()'s agreement rate is ever used to inform decay.

Synthesis: the literature leans toward decay conditioned on step *outcome patterns* (clustering, trajectory shape, external verification) rather than a pure single-parameter exponential half-life over wall-clock time alone. If implementing the exponential variant, consider whether `confidence_at()` should weight recent clustered failures more heavily than the same failure count spread out, and whether an external signal (multi-agent deliberation, doublecheck results — see doublecheck-confidence-adjustment) should be able to adjust the curve directly rather than relying on elapsed time as the only input.
