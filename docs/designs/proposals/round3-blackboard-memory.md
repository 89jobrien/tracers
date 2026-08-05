---
title: Blackboard memory with provenance
slug: blackboard-memory
round: 3
status: draft
viability: medium
depends_on:
- trace-graph
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Blackboard memory with provenance

## Problem

Multi-agent pipelines that need shared scratchpad state today mean awkward Input-type plumbing or an invisible Arc<Mutex<..>> side channel that defeats provenance capture.

## Approach

`Blackboard` wrapping Arc<RwLock<HashMap<String, serde_json::Value>>>; every read/write goes through AgentContext::record_step so it appears in the causal chain.

## API sketch

`struct Blackboard { entries: Arc<RwLock<HashMap<String, serde_json::Value>>> }`; `impl Blackboard { async fn write(&self, key: &str, value: serde_json::Value, ctx: &mut AgentContext); async fn read(&self, key: &str, ctx: &mut AgentContext) -> Option<serde_json::Value> }`

## Integration

TraceGraph becomes the natural place to visualize blackboard reads/writes as edges between agents with no direct Input/Output relationship.

## Verification notes

Confirmed AgentContext::record_step() is pub (crates/agent/src/context.rs) — routing blackboard access through it is legitimate today, no new AgentContext API needed.

## Dependencies

- trace-graph

## Notes

Open risk: does blackboard I/O count against the same step budget as an agent's actual work, or does it need separate accounting? Decide before implementation — a chatty blackboard pattern could silently exhaust budget.

## Prior art

- **Hearsay-II** (Erman, Hayes-Roth, Lesser, Reddy, ACM Computing Surveys, 1980, https://dl.acm.org/doi/10.1145/356810.356816) and **Hayes-Roth's "A Blackboard Architecture for Control"** (Artificial Intelligence 26, 1985, https://www.sciencedirect.com/science/article/abs/pii/0004370285900633) — the original blackboard model this proposal is named after. Important correction: the classical model has **no inherent audit trail** — Hearsay-II and its successors avoided the concurrent-write problem entirely by construction (a scheduler serialized which knowledge source fired next), not by solving it. There is no historical precedent that "let concurrent writes race, then trace them" was ever validated — the classical systems never actually faced that problem.
- **PatchBoard: Schema-Grounded State Mutation for Reliable and Auditable LLM Multi-Agent Collaboration** (arXiv:2605.29313, 2026) — the one modern system found that treats this exact problem (unvalidated shared-state mutation silently corrupting downstream agents) as a first-class design concern. Its answer is stronger than this proposal's sketch: agents *propose* JSON Patch mutations against a schema; a deterministic kernel validates each patch against registered invariants **before** committing (rejecting malformed/unauthorized writes via role-based write contracts), and logs every accepted *and* rejected patch with full attribution. Reports 84.6% task success vs. 30.8% for a LangGraph baseline with zero write-contamination. This is real, load-bearing evidence, not a hunch.
- Two other 2025 LLM-multi-agent papers use "blackboard" as a coordination metaphor (arXiv:2507.01701, arXiv:2510.01285 — agents self-select which to act based on shared content) but neither specifies a conflict-resolution or audit mechanism in the available text — they're architectural inspiration, not concurrency-safety precedent.

**This changes the design**: the sketch's plain `Arc<RwLock<HashMap<..>>>` with "last write wins, but every access is traced" is a reasonable *starting point* but is weaker than the one system (PatchBoard) that actually engineered for this problem and measured the result. Logging alone wasn't sufficient in their findings — they needed schema/invariant validation *before* commit, not just after-the-fact traceability. Recommend at minimum a `WriteResult` return type from `Blackboard::write()` that can reject (not just log) a write that violates a caller-supplied invariant, rather than pure last-write-wins.
