---
title: health-score-weighted delegation routing
slug: health-score-weighted-routing
round: 4
status: draft
viability: medium
depends_on:
- namespaced-agent-registry
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# health-score-weighted delegation routing

## Problem

NamespacedRegistry resolves by namespace alone, with no way to route based on the live condition (health-score trend) of the code an agent is about to touch.

## Approach

`get_health_aware(name, target_crate)` checks health_score::trend_for() and routes to a `{name}-cleanup` variant when a crate's health is declining.

## API sketch

`trait HealthAwareRouting<I, O> { fn route(&self, name: &str, target_crate: &str) -> Option<Arc<dyn Agent<Input=I,Output=O>>> }`; `impl<I,O> AgentRegistry<I,O> { fn get_health_aware(&self, name: &str, target_crate: &str) -> Arc<dyn Agent<Input=I,Output=O>> }`

## Integration

Extends namespaced-agent-registry with health-score data as an additional routing axis.

## Verification notes

godmode's health-score skill confirmed to exist as a real skill/agent (found via godmode agents directory search), same correction as mistake-tracker-crossref — this is a godmode skill, not a standalone tool as originally framed.

## Dependencies

- namespaced-agent-registry

## Notes

Decide whether missing a specialized variant (e.g. no 'Coder-cleanup' registered) silently falls through to the base agent or is a configuration error — silent fallthrough risks routing a declining crate to an inappropriate generic agent.

## Prior art
No dedicated research agent was run for this one — this is an internal-integration proposal
between two local, already-owned systems (namespaced-agent-registry and the godmode health-score
skill). The open design question (silent fallthrough vs. hard error when a specialized routing
variant is missing) is a local API-design tradeoff, not something external literature resolves.
