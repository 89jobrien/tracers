---
title: Capability-typed tools
slug: capability-typed-tools
round: 2
status: draft
viability: low
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Capability-typed tools

## Problem

tool declarations are typed by input/output shape but nothing constrains which agents may call which tools at compile time.

## Approach

Phantom `Cap<C>` capability markers; `HasCap<C>` trait bound gates tool methods; agents declare grants via a `Grants` type parameter.

## API sketch

`struct Cap<C> { _capability: PhantomData<C> }`; marker types `NetworkAccess`/`FileWrite`/`ShellExec`; `trait HasCap<C> {}`; `trait RequiresCap<C> {}` on tool types; agent structs generic over `Grants: HasCap<C>`

## Integration

Sits in tracers-agent next to Agent. AgentRegistry in tracers-runtime would need to become capability-aware.

## Verification notes

No `Tool` trait exists anywhere in this codebase today — Agent exists; tool is still prose in the language design (README), not a Rust type.

## Notes

Should follow, not precede, an actual Tool trait landing in tracers-agent. See governance-audit-reconciliation (round 5) — godmode-agent-governance already has a working runtime PolicyEngine (policy composition, tool wrappers, rate limiting) that answers this proposal's own open question about the dynamic half.

## Prior art

Shares its research grounding with trust-provenance (round 1) — see that doc's Prior art section for the full IFC/capability-theory citations (Jif, "Capability Myths Demolished," Filament). Key points specific to this proposal:

- **Object-capability model** (E language, Cap'n Proto RPC — https://capnproto.org/rpc.html) — the classical model this proposal's `Cap<C>` markers borrow from: capabilities as unforgeable references passed via normal parameter-passing, with no ambient authority. Known, well-documented limitations apply directly here: **capability leakage** (once an agent holds a `Cap<NetworkAccess>`, nothing in the base model stops it from being forwarded to code that shouldn't have it, absent an explicit confinement layer like E's "membrane" pattern) and **revocation difficulty** (pure capabilities aren't revocable by default — revoking a grant mid-run would need an indirection/proxy layer this sketch doesn't have).
- **"Lingering Authority: Revocable Resource-and-Effect Capabilities for Coding Agents"** (arXiv:2606.22504, 2026) — the closest existing work to "capabilities for agent tool-calling," but it's a **runtime**, lease-based, revocable capability system, not compile-time typing. This is a meaningfully different tradeoff: runtime capabilities can be revoked mid-session and audited without recompilation; compile-time phantom types (this proposal) cannot be revoked at all once an agent is constructed with a `Grants` type — the grant is fixed for the agent's lifetime in the type system.
- No paper or crate found doing compile-time/type-level tool-capability gating for LLM agents specifically — this remains a genuinely open, unvalidated application, same finding as trust-provenance.

**Bottom line**: same as trust-provenance — the type-level mechanism is well-precedented by 25+ years of IFC/capability literature, but note one real tradeoff the research surfaced that the original sketch doesn't address: compile-time capabilities can't be revoked at runtime the way the one real LLM-agent capability system found (Lingering Authority) can. If revocation matters (e.g. a human needs to pull a grant mid-session), this proposal needs a runtime companion — which is exactly what governance-audit-reconciliation's PolicyEngine already provides — not a replacement for it.
