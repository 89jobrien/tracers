---
title: TraceErr/Step as the audit-trail format for godmode:agent-governance
slug: governance-audit-reconciliation
round: 5
status: draft
viability: medium
depends_on:
- confidence-decay
- capability-typed-tools
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# TraceErr/Step as the audit-trail format for godmode:agent-governance

## Problem

godmode-agent-governance already covers policy composition, tool wrappers, trust decay, and JSONL audit trails; trace-core's TraceErr/Step independently record comparable data — two disconnected records of the same governed run.

## Approach

Every policy check (allow or deny) becomes a Step, so trace.causal_chain() already is the audit trail governance needs.

## API sketch

`fn enforce_policy(policy: &Policy, action: &Action, ctx: &mut AgentContext) -> Result<(), TraceErr>` — calls ctx.record_step() and returns Err(TraceErr::other(...)) on denial.

## Integration

Runtime complement to capability-typed-tools (compile-time half); reconciles trust decay (governance) with confidence-decay (round one) under different vocabulary for the same exponential-decay mechanism.

## Verification notes

CONFIRMED via direct read of godmode/skills/agent-governance/SKILL.md: this is a real, mature, production system — not aspirational prose. TrustScore::current(decay_rate) = score * exp(-decay_rate * elapsed) is materially the same formula family as confidence-decay's DecayCurve sketch. Policy composition (Pattern 2, most-restrictive-wins) and governed tool wrappers (Pattern 3) directly answer capability-typed-tools' own open question about a dynamic/runtime policy engine. JSONL audit trail (Pattern 5) is append-only, matching the compliance requirement TraceErr/Step would need to genuinely replace it.

## Dependencies

- confidence-decay
- capability-typed-tools

## Notes

Read this skill's Rust patterns section directly before building confidence-decay or capability-typed-tools — real risk of shipping a worse reimplementation of something already working in production. This is the single most important finding across all six rounds.

## Prior art
Already has the strongest verification of any proposal in this set — see this doc's own
Verification notes, which quote godmode-agent-governance's real TrustScore::current(decay_rate)
exponential-decay formula directly from its SKILL.md. That formula's shape is corroborated by the
IFC/capability-theory research done for trust-provenance and capability-typed-tools (round 1/2 —
governance's policy-composition and tool-wrapper patterns are a working instance of exactly the
runtime PolicyEngine those docs' research found missing from the compile-time-only phantom-type
approach). No additional external research was run beyond the direct SKILL.md read already
performed; this proposal's grounding is internal-verification-driven, and that's its actual
strength — reading the real skill file was more valuable here than a literature search would be.
