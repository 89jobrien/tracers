---
title: Step cost ledger
slug: step-cost-ledger
round: 1
status: draft
viability: high
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Step cost ledger

## Problem

trace:: records what happened but not what it cost (tokens, dollars) — cost tracking lives in a separate log-scraping system.

## Approach

`StepCost { input_tokens, output_tokens, dollars: Option<f64> }` attached per-step; `Trace::total_cost()`/`priciest_steps()` fold/sort, same shape as the existing `bottlenecks()`.

## API sketch

`struct StepCost { input_tokens: u64, output_tokens: u64, dollars: Option<f64> }` (impl Add); `impl Step { fn with_cost(mut self, cost: StepCost) -> Self }`; `impl<T> Trace<T> { fn total_cost(&self) -> StepCost; fn priciest_steps(&self) -> Vec<&Step> }`

## Integration

Zero new crates, zero new traits — same-shape addition to code that already exists (bottlenecks() is the direct template).

## Verification notes

Confirmed against crates/core/src/step.rs and trace.rs — with_confidence/with_duration/bottlenecks() are the real precedent.

## Notes

Ship first. Lowest risk feature in the entire proposal set.

## Prior art

- **OpenTelemetry GenAI semantic conventions** (via https://opentelemetry.io/docs/specs/semconv/gen-ai/, corroborated by secondary sources since the canonical page could not be pulled directly) — **token counting is not novel**: OTel already standardizes `gen_ai.usage.input_tokens`/`output_tokens` as span attributes plus a `gen_ai.client.token.usage` histogram metric (GenAI SIG formed April 2024, mature by 2026). If `StepCost` only counts tokens, it's reimplementing an existing convention, just as a typed Rust field instead of an untyped attribute bag.
- **Cost-in-currency is genuinely different ground**: none of the sources found show OTel defining a `gen_ai.usage.cost` or currency-denominated attribute anywhere — every cost-tracking approach found describes cost as computed *downstream* by multiplying token counts by an externally-known price table, never emitted or schema'd by OTel itself. `StepCost.dollars: Option<f64>` as a typed, first-class field is therefore real differentiation, not a reinvention — consistent with this doc's own open question about whether dollars should be stored or computed lazily against a pricing table.

**Bottom line**: keep the token-count fields (they're validating an existing, sound convention, not novel), but the `dollars` field is where this proposal actually adds value beyond what's already standardized. No change to viability — still ship first — but frame the token fields as "matching an established convention" and the dollar field as the genuinely new part when documenting this.
