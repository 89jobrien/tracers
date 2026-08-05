---
title: Failure-learning as confidence decay's reference implementation
slug: failure-learning-decay-reference
round: 6
status: draft
viability: high
depends_on:
- confidence-decay
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Failure-learning as confidence decay's reference implementation

## Problem

confidence-decay (round one) left the decay curve shape as an open design question; coursers already ships a working, tuned answer to a closely related problem.

## Approach

Reuse coursers' failure_learning config (window_seconds, block_threshold, cleanup_after_seconds) as a threshold-based DecayCurve variant, alongside (not necessarily replacing) round one's exponential half-life sketch.

## API sketch

`struct DecayCurve { window: Duration, block_threshold: usize, cleanup_after: Duration }` (threshold-based, distinct from the exponential half-life variant)

## Integration

Directly resolves confidence-decay's open question about decay curve shape and default parameters using values already tuned through real production use.

## Verification notes

CONFIRMED via direct read of ~/dev/coursers/README.md: the exact config schema and default values (block_threshold: 3, window_seconds: 300, cleanup_after_seconds: 3600, max_tracked_commands: 200) are real and match the proposal precisely.

## Dependencies

- confidence-decay

## Notes

Best-grounded proposal across all six rounds — not just plausible, the exact tuned constants exist. Important correction: this is threshold-based BLOCKING, not continuous decay — Step::confidence staying a smooth float doesn't map onto 'blocked after N failures.' Treat as a second, separate DecayCurve variant rather than assuming one model subsumes the other.

## Prior art

Non-arxiv industry precedent is the stronger grounding here: Hystrix, resilience4j, and Polly circuit breakers are all fundamentally sliding-window count/ratio thresholds, the same family as coursers' failure_learning — none of them use continuous exponential decay as the primary trip signal. One concrete gap this proposal should fix relative to that precedent: Hystrix's `requestVolumeThreshold`, resilience4j's `minimumNumberOfCalls`, and Polly's `MinimumThroughput` all guard against tripping on a low-sample-size false positive (e.g. 3 failures out of 3 attempts vs. 3 out of 300) — coursers' raw `block_threshold` count has no equivalent minimum-volume gate. Worth adding one when generalizing this beyond coursers' narrow shell-command use case.

### Trust/reputation decay literature (2022-2026)

- **An analysis of the exponential decay principle in probabilistic trust models** (ElSalamouny, Krukow, Sassone, *Theoretical Computer Science* 410(41), 2009 — not arxiv, pre-2022 but the only rigorous formal treatment found) — models principal behavior with Hidden Markov Models and derives an analytical error bound for exponential decay applied to Beta-distribution trust estimates. Frames decay as a bias/variance tradeoff (a fixed-form approximation to more complex dynamics), not a proof that exponential decay is optimal — relevant caveat against assuming exponential decay is automatically "more correct" than a tuned threshold model like coursers'.
- **A Survey of Multi-Agent Trust Management Systems** (Granatyr et al., ACM Computing Surveys / IEEE) — confirms temporal decay is a standard, expected component of MAS trust models generally, without adjudicating which decay-model family is superior.
- **DynaTrust: Defending Multi-Agent Systems Against Sleeper Agents via Dynamic Trust Graphs** (arXiv:2603.15661, 2026) — motivates why a purely cumulative/slowly-decaying trust score is exploitable (an agent can build credit through good behavior, then defect). A threshold-based blocking model like coursers' is naturally resistant to this specific failure mode, since it reacts sharply to a burst of recent failures rather than smoothing them into a slowly-moving average — a point in favor of keeping the threshold variant rather than replacing it with pure exponential decay.
- **Trust Between AI Agents: Measuring Formation, Breakage, and Recovery** (arXiv:2606.14923, 2026) — clustered failures matter more than the same failure count spread over time, which is structurally close to what a sliding-window threshold already captures (a burst within `window_seconds` trips the block; the same count spread across weeks does not, once `cleanup_after_seconds` expires old entries).
- **The Trust Paradox in LLM-Based Multi-Agent Systems** (arXiv:2510.18563, 2025) — argues that raising inter-agent trust to improve coordination linearly increases over-exposure/over-authorization security risk, supporting some form of decay/re-verification rather than monotonically accumulating trust — relevant motivation, though it doesn't specify a decay model.

No paper (2022-2026, arxiv or otherwise) was found directly running a head-to-head empirical comparison of threshold/windowed vs. exponential decay for this exact use case — the strongest evidence for this proposal remains the non-arxiv industry precedent (Hystrix/resilience4j/Polly/Envoy) rather than academic literature, with the 2025-2026 MAS-trust papers above providing indirect support for keeping a threshold-reactive component rather than relying on smooth decay alone.
