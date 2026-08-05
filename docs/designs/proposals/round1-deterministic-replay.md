---
title: Deterministic replay & trace diffing
slug: deterministic-replay
round: 1
status: draft
viability: medium-low
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Deterministic replay & trace diffing

## Problem

No structured way to answer 'what actually changed' when a prompt/model/tool changes — correlation is manual (git blame + eyeballing timestamps).

## Approach

`Step::with_seed(seed: u64)`; `Trace::is_reproducible()`; `replay(agent, original, input) -> (Trace<O>, TraceDiff)` re-runs and diffs causal chains.

## API sketch

`impl Step { fn with_seed(mut self, seed: u64) -> Self }`; `impl<T> Trace<T> { fn is_reproducible(&self) -> bool }`; `struct TraceDiff { added_steps, removed_steps, changed_confidence, value_changed }`; `async fn replay<A: Agent>(agent: &A, original: &Trace<A::Output>, input: A::Input) -> (Trace<A::Output>, TraceDiff)`

## Integration

Own crate (trace-replay), depends on tracers-agent's Agent trait. TaskRegistry checkpoints are the natural source of 'original' traces.

## Verification notes

No provider-level seed/determinism plumbing exists anywhere in tracers-agent today — Agent::run has no hook for deterministic sampling.

## Notes

is_reproducible() would be honest-but-mostly-useless until an actual provider integration exists to thread seeds through LLM calls. Blocked on infrastructure this repo doesn't have yet.

## Prior art

- **`rr` (Mozilla record-replay debugger)** (https://rr-project.org/) — the canonical proof that "record only the nondeterministic boundary inputs, replay the rest deterministically" is viable — but even `rr` needs to control the *entire* execution environment (syscalls, CPU cycle counts via hardware performance counters) to do it, and still can't handle processes sharing memory outside the recording tree or arbitrary CPU generations. **Deterministic Replay: A Survey** (ACM Computing Surveys 48:2, https://dl.acm.org/doi/10.1145/2790077) confirms full-fidelity replay is prohibitively expensive without narrowing scope even for classical software — this is a hard problem in general, not just for LLMs.
- **Defeating Nondeterminism in LLM Inference** (Thinking Machines Lab, https://thinkingmachines.ai/blog/defeating-nondeterminism-in-llm-inference/) — the single most important finding: temperature=0 nondeterminism is primarily caused by **batch-size-dependent reduction order** in server-side kernels, not floating-point non-associativity as commonly assumed. Server-side batch composition depends on concurrent load from *other* callers, which is invisible and uncontrollable from the API caller's side. **No seed value can compensate for this** — it's a provider-infrastructure-level source of nondeterminism, confirmed as still being active research (see also arXiv:2601.17768, "LLM-42," a 2026 attempt to fix it at the serving layer, and arXiv:2601.06118, a 2026 paper on detecting this nondeterminism via token-probability inspection).
- **OpenAI Cookbook — seed parameter** (https://cookbook.openai.com/examples/reproducible_outputs_with_the_seed_parameter) — OpenAI's own documentation states outputs are "mostly deterministic," not guaranteed, even with matched seed. Requires also matching `system_fingerprint`, which changes silently "a few times a year" — a stored seed from a past run may become **unreplayable in principle** months later, independent of client effort. Anthropic does not publicly document a seed parameter for the Messages API at all.
- No paper found doing exactly "record/replay + causal diff of two agent runs against a fixed seed" as a general library primitive (closest are arXiv:2606.07054 "TRACE" and arXiv:2605.21347, both single-trajectory diagnostics, not two-run diffing) — this specific mechanism appears to be a genuine gap in the literature, not a solved problem being reinvented.

**This changes the design, not just the confidence rating**: `is_reproducible() -> bool` is a claim the underlying infrastructure cannot honestly support — even a matched seed on the same provider is only "mostly" deterministic, and Anthropic doesn't expose a seed at all. Rename/reframe before implementing: something like `reproducibility_confidence() -> f64` or a diff-based drift score against the prior trace, not a boolean guarantee. The real value of this feature is `TraceDiff` attributing *where* two runs diverged, not a promise of bitwise reproducibility as a precondition for using it.
