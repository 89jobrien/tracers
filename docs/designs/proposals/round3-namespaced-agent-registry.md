---
title: Namespaced AgentRegistry
slug: namespaced-agent-registry
round: 3
status: draft
viability: medium
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Namespaced AgentRegistry

## Problem

AgentRegistry is a single flat namespace keyed by Agent::name() — the same escalation name ("Reviewer") can't resolve differently per tenant/workspace.

## Approach

`NamespacedRegistry` wraps one AgentRegistry per namespace plus a fallback default namespace.

## API sketch

`struct NamespacedRegistry<I, O> { namespaces: HashMap<String, AgentRegistry<I, O>>, default_namespace: String }`; `impl NamespacedRegistry<I,O> { fn register(&mut self, namespace: &str, agent: Arc<dyn Agent<Input=I,Output=O>>); fn get(&self, namespace: &str, name: &str) -> Option<Arc<dyn Agent<Input=I,Output=O>>> }`

## Integration

Purely additive wrapper around the existing AgentRegistry — confirmed real, plain HashMap-backed struct in crates/runtime/src/registry.rs.

## Verification notes

Confirmed AgentRegistry's real shape. But the original proposal's claim that run_with_escalation's signature 'grows a namespace argument, defaulting to default' is not how Rust works — there is no default-parameter mechanism.

## Notes

Fix before implementing: add a new run_with_escalation_namespaced function rather than attempting to change the existing pub fn run_with_escalation's signature (confirmed real, with existing callers/tests in execute.rs).

## Prior art
No dedicated research agent was run for this one. Namespaced name resolution with a fallback
default is an extremely standard pattern (DNS search domains, Kubernetes namespaces, most
service-discovery systems) with no meaningful research literature specific enough to cite. The
real issue this doc already found — that Rust has no default-parameter mechanism, so the original
proposal's signature-change claim doesn't hold — is a language-mechanics fact, not something
external research bears on.
