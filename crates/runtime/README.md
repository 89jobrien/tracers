# trace-lang-runtime

[![crates.io](https://img.shields.io/crates/v/trace-lang-runtime.svg)](https://crates.io/crates/trace-lang-runtime)
[![docs.rs](https://docs.rs/trace-lang-runtime/badge.svg)](https://docs.rs/trace-lang-runtime)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://github.com/89jobrien/trace-lang)

Agent registry, delegation resolution, parallel fan-out, and speculative
branching for `trace::` pipelines.

`trace-lang-agent` defines *what* an agent should do when it needs to escalate
(`EscalationAction::Delegate("SeniorCoder")`), but resolving a name into a live
agent and actually running it is a runtime concern — that's what this crate
adds: `AgentRegistry` (name → agent lookup), `run_with_escalation` (auto-resolve
delegation up to a hop limit), `join_all` (fan a single agent out over many
inputs), and `speculate` (race several different agents, keep the most
confident).

## Install

```toml
[dependencies]
trace-lang-runtime = { path = "../runtime" }
```

## `AgentRegistry<I, O>`

A name → `Arc<dyn Agent<Input = I, Output = O>>` lookup table. All agents
registered under one `AgentRegistry<I, O>` must share the same `Input`/`Output`
contract, since a delegation target (a reviewer, a fallback, a specialist)
needs to accept the same task shape as the agent that escalated to it.

```rust
use trace_lang_runtime::AgentRegistry;
use trace_lang_agent::{Agent, AgentContext};
use trace_lang_core::Trace;
use async_trait::async_trait;
use std::sync::Arc;

struct Fallback;

#[async_trait]
impl Agent for Fallback {
    type Input = String;
    type Output = String;
    fn name(&self) -> &str { "Fallback" }
    fn goal(&self) -> &str { "handle what the primary agent could not" }
    async fn run(&self, input: String, ctx: &mut AgentContext) -> Trace<String> {
        ctx.record_step().unwrap();
        Trace::new(format!("fallback handled: {input}"))
    }
}

let mut registry: AgentRegistry<String, String> = AgentRegistry::new();
registry.register(Arc::new(Fallback));
assert!(registry.get("Fallback").is_some());
assert!(registry.get("Unknown").is_none());
```

| method | returns | purpose |
| --- | --- | --- |
| `new()` | `Self` | empty registry |
| `register(agent)` | — | insert under `agent.name()`, overwriting any existing entry |
| `get(name)` | `Option<Arc<dyn Agent<...>>>` | lookup by name |
| `contains(name)` | `bool` | |
| `len()` / `is_empty()` | `usize` / `bool` | |

## `run_with_escalation`

```rust
pub async fn run_with_escalation<I, O>(
    agent: &dyn Agent<Input = I, Output = O>,
    input: I,
    registry: &AgentRegistry<I, O>,
    max_hops: usize,
) -> RunOutcome<O>
where
    I: Clone + Send,
    O: Clone + Serialize + Send;
```

Runs `agent`; if its lifecycle hooks recommend delegating, resolves that
delegation against `registry` and keeps going — up to `max_hops` handoffs —
until a run produces no further escalation, the registry can't resolve the
named target, or the hop limit is reached. `input` must be `Clone`: each hop
re-runs the *same* task against a new agent (retry the original task with a
different agent, not continue from partial output).

```rust
use trace_lang_agent::{Agent, AgentContext, EscalationAction};
use trace_lang_core::Trace;
use async_trait::async_trait;
use std::sync::Arc;

struct Junior;
#[async_trait]
impl Agent for Junior {
    type Input = u32;
    type Output = u32;
    fn name(&self) -> &str { "Junior" }
    fn goal(&self) -> &str { "attempt the task, escalate on failure" }
    async fn run(&self, _input: u32, ctx: &mut AgentContext) -> Trace<u32> {
        ctx.record_step().unwrap();
        Trace::failed(trace_lang_core::TraceErr::other("out of my depth"))
    }
    fn on_step_failure(&self) -> EscalationAction {
        EscalationAction::Delegate("Senior".to_string())
    }
}

struct Senior;
#[async_trait]
impl Agent for Senior {
    type Input = u32;
    type Output = u32;
    fn name(&self) -> &str { "Senior" }
    fn goal(&self) -> &str { "handle what Junior escalated" }
    async fn run(&self, input: u32, ctx: &mut AgentContext) -> Trace<u32> {
        ctx.record_step().unwrap();
        Trace::new(input * 2)
    }
}

let mut registry: AgentRegistry<u32, u32> = AgentRegistry::new();
registry.register(Arc::new(Senior));

let outcome = run_with_escalation(&Junior, 21, &registry, 3).await;
assert_eq!(outcome.trace.value(), Some(&42));
assert!(outcome.unresolved.is_none());
assert_eq!(outcome.context.delegation_chain, vec!["Junior", "Senior"]);
```

`RunOutcome<O>`:

| field | type | meaning |
| --- | --- | --- |
| `trace` | `Trace<O>` | final trace produced |
| `context` | `AgentContext` | final context, including the full `delegation_chain` |
| `unresolved` | `Option<EscalationAction>` | `Some` if the run stopped with an escalation still pending — `max_hops` reached, or the registry doesn't recognize the target. `None` means the chain terminated cleanly |

## `join_all`

```rust
pub async fn join_all<A>(agent: &A, inputs: Vec<A::Input>) -> Vec<SpawnOutcome<A::Output>>
where
    A: Agent + ?Sized;
```

Runs one agent concurrently over many inputs, collecting one `SpawnOutcome` per
input in the original order.

```rust
let outcomes = join_all(&Researcher, topics).await;
```

## `speculate`

```rust
pub async fn speculate<I, O>(
    candidates: Vec<(String, Arc<dyn Agent<Input = I, Output = O>>)>,
    input: I,
) -> Trace<O>
where
    I: Clone + Send,
    O: Clone + Serialize + Send;
```

Runs several *different* candidate agents concurrently against the same input,
picks a winner, and appends a single `"speculate"` step to the winning trace
whose `Branch`es record which candidate was taken and which were rejected (and
why).

```rust
let candidates: Vec<(String, Arc<dyn Agent<Input = _, Output = _>>)> = vec![
    ("aggressive".into(), Arc::new(AggressivePlanner)),
    ("conservative".into(), Arc::new(ConservativePlanner)),
];
let trace = speculate(candidates, task).await;
// losing candidates are recorded as rejected Branches on a "speculate" step
```

**Scoring:** a candidate's confidence is the mean of its step confidences
(steps with no recorded confidence are ignored; zero scored steps counts as
`0.0`). A candidate that produced a `TraceErr` scores `-1.0`, so it never
outranks a successful candidate — its rejection reason is the error's
`Display` text.

**Ties:** the winner is picked by a manual fold over scores
(`first_max_index`), not `Iterator::max_by` — `max_by` returns the *last*
tied element, which would silently break "first candidate wins ties"
semantics. This exact bug shipped once; `first_max_index` is now covered by
unit tests, a `proptest` property test, and a `kani` proof
(`first_max_index_never_indexes_out_of_bounds`,
`first_max_index_on_two_element_tie_keeps_first`) that hold the invariant
exhaustively within bounds.

**Panics:** `speculate` panics if `candidates` is empty — there is nothing to
speculate over.

## Known limitation

`join_all` and `speculate` both use `futures::future::join_all`, which polls
concurrently on the current task rather than distributing across OS threads.
True multi-threaded parallelism (via `tokio::spawn` and `'static` agents,
typically `Arc<dyn Agent<...>>` throughout) and a shared step-budget spanning
concurrent branches are both tracked as open follow-ups, not silently assumed —
see this repo's root `CLAUDE.md` ("deferred") and `.ctx/HANDOFF.md`.

## Testing

```bash
cargo test -p trace-lang-runtime
```
