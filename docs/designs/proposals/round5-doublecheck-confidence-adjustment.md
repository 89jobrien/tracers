---
title: godmode:doublecheck as a confidence-adjusting trace pass
slug: doublecheck-confidence-adjustment
round: 5
status: draft
viability: medium-high
depends_on:
- confidence-calibration
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# godmode:doublecheck as a confidence-adjusting trace pass

## Problem

A step's confidence is purely self-reported; doublecheck already does adversarial claim verification but the two systems don't share data.

## Approach

`Step::with_doublecheck(result)` adjusts confidence by the supported/extracted claim ratio and records the adjustment explainably in notes.

## API sketch

`struct DoublecheckResult { claims_extracted: usize, claims_supported: usize, claims_contradicted: usize, adversarial_flags: Vec<String> }`; `impl Step { fn with_doublecheck(mut self, result: DoublecheckResult) -> Self }`

## Integration

Concrete ground-truth source for confidence-calibration's previously-open 'where does ground truth come from' question.

## Verification notes

Confirmed godmode:doublecheck is real (used directly earlier in this conversation) with exactly the claim-extraction/verification report shape described.

## Dependencies

- confidence-calibration

## Notes

Most trace:: steps (a tool call, a delegation) don't have 'claims' in the sense doublecheck checks — scope this to LLM-generated factual output steps only, and consider gating it to already-low-confidence steps rather than running on every step given real per-call latency/token cost.

## Prior art
Shares its research grounding with confidence-decay (round 1) — see that doc's Prior art section
for the LLM-calibration literature (Kadavath, Tian et al., multi-agent-deliberation calibration,
2026 agentic-calibration papers). No additional research was run specifically for this proposal
beyond confirming godmode:doublecheck is a real, working skill (done directly in this conversation
by running it). The scoping question this doc already raises — which steps have "claims" doublecheck
can meaningfully check — is a local design decision, not something the calibration literature
resolves generically.
