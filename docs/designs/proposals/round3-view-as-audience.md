---
title: "Trace::view_as(role) \u2014 audience-scoped projections"
slug: view-as-audience
round: 3
status: draft
viability: low
depends_on:
- trace-narrate
- redaction-aware-traces
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Trace::view_as(role) — audience-scoped projections

## Problem

Redaction is binary (safe/not safe); real audiences (customer, engineer, auditor) need different projections of the same trace, currently requiring separately-maintained rendering code per consumer.

## Approach

`Audience` enum with `Trace::view_as(audience) -> TraceView`, each variant composing narrate()/full causal chain/unredacted fields as appropriate.

## API sketch

`enum Audience { Customer, Engineer, Auditor }`; `enum TraceView { Narrative(String), Full { chain: Vec<Step>, rejected: Vec<Step> }, FullUnredacted { chain: Vec<Step>, redacted_fields: Vec<RedactedField> } }`; `impl<T> Trace<T> { fn view_as(&self, audience: Audience) -> TraceView }`

## Integration

Composes trace-narrate (Customer view) and redaction-aware-traces (Engineer vs Auditor distinction) into one decision point.

## Verification notes

No new mechanism, purely compositional — but both dependencies are themselves unshipped.

## Dependencies

- trace-narrate
- redaction-aware-traces

## Notes

Cannot be scheduled before trace-narrate and redaction-aware-traces both land. Authorization is explicitly out of scope — view_as is a pure data-shaping function that trusts the caller to have already verified the audience is legitimate.

## Prior art
No dedicated research agent was run for this one. Audience-scoped data projection is standard
practice (RBAC-gated views, GraphQL field-level authorization, API response shaping per client
tier) with no specific research literature to cite beyond general access-control theory already
covered by the capability-typed-tools and trust-provenance docs' IFC/capability-model research.
This proposal is a pure composition of two other docs (trace-narrate, redaction-aware-traces) —
its viability is bounded by theirs, not by anything external.
