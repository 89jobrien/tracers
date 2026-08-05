---
title: "Trace<T, Trust> \u2014 type-level provenance"
slug: trust-provenance
round: 1
status: draft
viability: low
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Trace<T, Trust> — type-level provenance

## Problem

Nothing stops a low-trust value from reaching code that assumes high-trust input; the check is a discipline, not a guarantee.

## Approach

Phantom type parameter `Trace<T, P = Unverified>` with marker types (LLMGenerated, ToolComputed, HumanVerified). Only `verified_by()` constructs HumanVerified.

## API sketch

`Trace<T, P>`; marker structs `Unverified`/`LLMGenerated`/`ToolComputed`/`HumanVerified`; `impl<T> Trace<T, LLMGenerated> { fn verified_by(self, reviewer: &str) -> Trace<T, HumanVerified> }`

## Integration

Cascades through Agent::Output, TaskStatus::Done(TraceRef), speculate<I,O>, TraceRef itself. TraceRef is stored in Task::status and must stay serializable — trust erasure at that exact boundary undermines the feature.

## Verification notes

Confirmed pervasive non-generic use of Trace<T> across the real codebase (Agent::Output, TaskStatus::Done(TraceRef), speculate). No code changes needed to confirm — this is a structural read of crates/core, crates/task, crates/agent, crates/runtime.

## Notes

Needs its own design pass on how TraceRef degrades before this is implementable — not simply an ergonomics tax as originally framed.

## Prior art

- **"Capability Myths Demolished"** (Miller, Yee, Shapiro, 2003, https://classpages.cselabs.umn.edu/Fall-2021/csci5271/papers/SRL2003-02.pdf) and **Jif** (Myers &amp; Liskov, "Protecting Privacy Using the Decentralized Label Model," ACM TOSEM 2000, https://www.cs.cornell.edu/andru/papers/iflow-tosem.pdf) — Jif's core pattern is **structurally identical to this proposal**: security labels attached to types, checked statically, with declassification as a single, explicit, syntactically-marked operation (`declassify(expr, newLabel)`) that requires the caller to hold specific authority — exactly `verified_by()`'s shape. This is not a novel invention; it's a 25-year-old, well-established IFC pattern (Sabelfeld &amp; Myers' survey "Language-Based Information-Flow Security" is the standard citation establishing "one narrow, audited escape hatch" as the field's converged design, not an outlier).
- **Filament: Denning-Style Information Flow Control for Rust** (arXiv:2604.14357, 2026) — a genuine, recent academic prototype bringing exactly this style of type-level security labeling to Rust specifically, citing the same Sabelfeld/Myers/Pottier-Simonet lineage. Confirms the pattern is actively being formalized for Rust today, not merely hypothetical — but it is a research prototype, not a published, battle-tested crate.
- **No existing published Rust crate** implements this pattern as a general-purpose library (checked: no mature `capability-rs`/`trust-level` crate found) — `PhantomData`-based typestate is a standard, proven Rust idiom generally (documented in Rust By Example), but its application to *security/trust labeling* specifically has no off-the-shelf precedent to borrow from directly.
- LLM-agent-specific capability research (arXiv:2606.22504 "Lingering Authority," arXiv:2509.22256 "Context Space") solves the adjacent problem — agent permission/capability management — but exclusively at **runtime**, not via a type system. No paper found doing compile-time, phantom-typed trust/capability gating specifically for LLM agent outputs.

**Bottom line**: the mechanism (phantom types + single declassification function) is proven and well-established by IFC theory — this is not the risky part. What's unproven is the specific application to LLM-agent output trust, which has essentially no precedent (Filament is the closest, and it's general-purpose, not agent-specific). Combined with the earlier verification finding (TraceRef's serialization/erasure problem at the TaskStatus boundary), this proposal's risk is concentrated in the application domain and the TraceRef integration, not in the core type-system technique.
