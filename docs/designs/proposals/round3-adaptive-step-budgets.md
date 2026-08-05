---
title: Adaptive step budgets
slug: adaptive-step-budgets
round: 3
status: draft
viability: medium
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Adaptive step budgets

## Problem

Agent::budget() is a single fixed number chosen at agent-definition time — it can't distinguish a simple task from a sprawling one.

## Approach

`BudgetPolicy<I>` trait replacing the fixed budget() default; evaluated once at spawn() time against the actual input.

## API sketch

`trait BudgetPolicy<I> { fn budget_for(&self, input: &I) -> usize }`; `struct FixedBudget(pub usize)`; `struct ProportionalBudget { base: usize, per_unit: usize }`

## Integration

spawn()/delegate() would call agent.budget_policy().budget_for(&input) instead of reading a static budget() value.

## Verification notes

Confirmed Agent::budget() (crates/agent/src/agent.rs:41) is called directly in TWO places — spawn.rs:38 and spawn.rs:61 (delegate) — both confirmed real. The trait's own test module (BudgetLimited) also overrides budget() and would need updating.

## Notes

Understated in the original proposal as 'small, localized' — it's a real breaking change across the Agent trait, both call sites, and every existing budget()-overriding impl. Scope as its own small plan with an explicit call-site list.

## Prior art

- **Adaptive Computation Time** (Graves, 2016, arXiv:1603.08983) and **Universal Transformers** (Dehghani et al., 2018, arXiv:1807.03819) — the classical lineage for input-dependent compute budgets: a *learned* per-input halting signal, not a heuristic over surface features like input length. This is the field's original and still-dominant framing.
- **SelfBudgeter** (arXiv:2505.11274, 2025) — directly relevant negative-ish finding: a model predicting its own token budget from *difficulty* (learned) achieves 61% length compression at equal accuracy versus naive length-based budgets, i.e. difficulty-conditioned budgets measurably beat length-proportional ones on real tasks.
- **Ares** (arXiv:2603.07915, 2026) and **Anytime Verified Agents** (OpenReview, https://openreview.net/forum?id=JMDCMf7mlF) — both explicitly frame *fixed/static* budget strategies as the baseline they beat, reinforcing that the field treats static or naive-heuristic budgets as the thing to move past, not the target design.
- **BudgetThinker** (arXiv:2508.17196) — notes that simple heuristics like "cap tied to prompt token count" are prone to abruptly stopping and incomplete work when the heuristic underestimates true difficulty — a direct, named risk for this proposal's `ProportionalBudget { base, per_unit }` sketch, which is exactly a length-proportional heuristic.
- **Reward hacking / proxy-gaming risk**: general framing (https://www.mdpi.com/2504-2289/3/2/21) plus arXiv:2604.15149 ("LLMs Gaming Verifiers") — any budget keyed off an easily-manipulated proxy (like raw input length) is a textbook Goodhart's-law setup: a caller or the agent itself could pad input to unlock a larger budget.

**This changes the design**: the proposal's own sketch (`ProportionalBudget`, length-proportional) is precisely the kind of heuristic the 2025-2026 literature treats as a weak starting point, not a good final design — real-world "thinking budget" knobs from LLM providers today are mostly caller-set fixed caps, while the research frontier is pushing toward learned/difficulty-conditioned allocation. Keep the pluggable `BudgetPolicy<I>` trait (that part is well-precedented and the right instinct — matches how the field frames this as needing pluggable policies, not one true heuristic), but flag `ProportionalBudget` as a naive default, and consider a policy variant driven by a cheaper difficulty proxy (e.g. prior-step confidence, an early-exit complexity check) rather than raw length alone. Guard whatever proxy is chosen against input-padding gaming, a documented failure mode.
