---
title: "TraceGraph \u2014 cross-trace lineage"
slug: trace-graph
round: 1
status: draft
viability: medium-low
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# TraceGraph — cross-trace lineage

## Problem

Trace::causal_chain() explains one run in isolation; nothing answers 'what's downstream of this trace' across a multi-agent pipeline.

## Approach

`TraceGraph` with nodes keyed by TraceRef and producer/consumer edges; `downstream_of`/`upstream_of`/`critical_path`.

## API sketch

`struct TraceGraph { nodes: HashMap<TraceRef, TraceNode>, edges: Vec<(TraceRef, TraceRef)> }`; `impl TraceGraph { fn record_edge(&mut self, producer: TraceRef, consumer: TraceRef); fn downstream_of(&self, t: &TraceRef) -> Vec<&TraceNode>; fn upstream_of(&self, t: &TraceRef) -> Vec<&TraceNode>; fn critical_path(&self) -> Vec<TraceRef> }`

## Integration

Task::depends_on already models this one level up (task level, not trace level) — confirmed real in crates/task/src/task.rs. But spawn()/delegate() in tracers-agent have no mechanism today to thread TraceRef identity between calls.

## Verification notes

Confirmed no producer/consumer trace-identity threading exists in spawn.rs/context.rs — this is not additive, it changes spawn/delegate's effective contract.

## Notes

Biggest surface-area proposal among the 'solid' round-one ideas — needs its own signature-change plan, not a drive-by.

## Prior art

- **W3C PROV-DM** (https://www.w3.org/TR/prov-dm/) — the standard provenance model. Uses a **typed tripartite** structure (Entity / Activity / Agent) connected by relations like `wasGeneratedBy`/`used`/`wasDerivedFrom`, not one undifferentiated node type. This proposal's sketch (`TraceGraph` with one `TraceNode` type per edge) is a simplification of PROV, not an equivalent — a `Trace<T>` conflates "the thing produced" (PROV's Entity) with "the process that produced it" (PROV's Activity). PROV has **no native critical_path concept** — that would need to be a bespoke graph algorithm (longest-path/topological analysis) layered on top, not something the provenance model itself provides.
- **PROV-AGENT: Unified Provenance for Tracking AI Agent Interactions in Agentic Workflows** (arXiv:2508.02866, 2025) — extends W3C PROV specifically for LLM agent workflows (tool calls, prompt/response, model invocations) as typed nodes with temporal/semantic edges. Closest existing academic analog, but focused on within-workflow lineage, not cross-trace lineage across separate pipeline runs.
- **From Agent Traces to Trust: A Survey of Evidence Tracing and Execution Provenance in LLM Agents** (arXiv:2606.04990) — notes most agent-trace schemas today capture prompts/tool calls/retrieval but few jointly represent inter-agent messages and cross-trace propagation — i.e. the gap this proposal targets is real and largely unaddressed in current practice, not just in this codebase.
- **OpenLineage** (https://github.com/OpenLineage/OpenLineage) and **Apache Atlas** (backed by JanusGraph) — production lineage systems both treat unbounded lineage-graph growth as a first-class problem, solved via graph-database backing (not in-memory `HashMap`) and summarization/sparsification of old subgraphs rather than unbounded retention. The sketch's `HashMap<TraceRef, TraceNode>` has no story for this yet.

Net assessment: the core "producer/consumer edges between execution units" idea is validated by PROV's real-world precedent, but two things the sketch is missing relative to that precedent are worth designing in early rather than retrofitting — (1) a typed distinction between the trace-as-process and its input/output values, and (2) a graph-growth/pruning story, since production lineage tooling treats that as unavoidable at scale, not optional.
