# trace::

> a programming language where reasoning is first-class

In most languages, values are first-class. In `trace::`, _reasoning provenance_ is first-class. Every computation returns `Trace<T>` — a value enriched with the full causal chain of how it was produced: steps taken, branches considered and rejected, confidence at each decision point, and RAII-style span timing.

This repository contains the `trace::` language design and a Rust reference implementation of its core types.

---

## crates

| crate           | description                                                     |
| --------------- | --------------------------------------------------------------- |
| `trace-core`    | `Trace<T>`, `Step`, `Span`, `Branch`, `TraceErr`                |
| `trace-task`    | `Task`, `TaskStatus`, `Priority`, `TaskRegistry`                |
| `trace-agent`   | `Agent` trait, `spawn`/`delegate`, lifecycle escalation hooks   |
| `trace-runtime` | `AgentRegistry`, `run_with_escalation`, `join_all`, `speculate` |

---

## the mental model

```rust
// in Rust you might write:
fn answer(q: Question) -> Answer { .. }

// in trace:: every computation is inspectable:
step answer(q: Question) -> Trace<Answer> { .. }

// and you can interrogate the result:
let t = spawn MyAgent::answer(q).await
t.causal_chain()       // how did we get here?
t.rejected_branches()  // what did we consider and discard?
t.bottlenecks()        // where did we slow down?
t.low_confidence()     // where were we uncertain?
```

`Trace<T>` is to reasoning what `Result<T, E>` is to errors: a typed, propagatable wrapper with explicit handling at every step. The `?` operator works identically — errors propagate as `TraceErr` and every propagation point is logged.

---

## task management

Tasks in `trace::` are serializable first-class values. `TaskStatus::Done` carries a `TraceRef` — a stable pointer back to the execution trace that produced the result. No output is ever detached from its provenance.

```rust
use trace_task::{Task, TaskRegistry, Priority};

let mut registry = TaskRegistry::new();

let t1 = Task::new("fetch requirements").with_priority(Priority::High);
let t2 = Task::new("plan architecture").depends_on(t1.id);

registry.insert(t1);
registry.insert(t2);

// only t1 is ready — t2 blocks on it
let ready = registry.ready_tasks();
```

The registry checkpoints after every task completion:

```rust
// after each task completes, save state
registry.complete(task.id, trace_ref, "./checkpoint.trace.json")?;

// resume a crashed executor — no task is re-run unnecessarily
let registry = TaskRegistry::load("./checkpoint.trace.json")?;
```

---

## agents (language design)

```
agent Planner {
  goal: "decompose a task into concrete, assignable steps"
  confidence: 0.8
  budget: 20 steps

  on_low_confidence  -> delegate(HumanReviewer)
  on_budget_exceeded -> emit(Err::BudgetExhausted)

  step plan(task: Task) -> Trace<Vec<Task>> {
    observe call_llm(decompose_prompt(task.goal))
    | parse_tasks()
    | assign_priorities()
    | resolve_dependencies()
  }
}
```

Key differences from a Rust `fn`:

| concept          | Rust fn | trace:: agent   |
| ---------------- | ------- | --------------- |
| return type      | `T`     | `Trace<T>`      |
| declares intent  | no      | yes (`goal:`)   |
| budget-limited   | no      | yes (`budget:`) |
| escalation rules | no      | yes (`on_*`)    |
| reasoning log    | no      | always-on       |

### the `trace-agent` crate

The `Agent` trait, `spawn`, and `delegate` are implemented today as a real (async-trait–backed) Rust API — not just language design:

```rust
use async_trait::async_trait;
use trace_agent::{Agent, AgentContext, EscalationAction, spawn, delegate};
use trace_core::Trace;

struct Coder;

#[async_trait]
impl Agent for Coder {
    type Input = String;
    type Output = String;

    fn name(&self) -> &str { "Coder" }
    fn goal(&self) -> &str { "produce correct, reviewed code" }
    fn confidence_threshold(&self) -> f64 { 0.8 }
    fn budget(&self) -> Option<usize> { Some(10) }

    async fn run(&self, spec: String, ctx: &mut AgentContext) -> Trace<String> {
        ctx.record_step().unwrap();
        Trace::new(format!("// implements: {spec}"))
    }

    fn on_low_confidence(&self) -> EscalationAction {
        EscalationAction::Delegate("SeniorCoder".to_string())
    }
    fn on_budget_exceeded(&self) -> EscalationAction {
        EscalationAction::Delegate("HumanReviewer".to_string())
    }
}
```

`spawn()` runs the agent and evaluates its lifecycle hooks against the resulting trace — budget exhaustion, step failure, and low confidence each resolve to an `EscalationAction` the caller can act on. `delegate()` does the same but extends the caller's `delegation_chain`, so a multi-agent handoff is always reconstructable from `AgentContext::delegation_chain`.

### the `trace-runtime` crate

`EscalationAction::Delegate("SeniorCoder")` is just a name — resolving it into a live agent and actually running it is a runtime concern. `trace-runtime` adds:

```rust
use trace_runtime::{AgentRegistry, run_with_escalation, join_all, speculate};
use std::sync::Arc;

// resolve escalations against a registry, hopping up to a limit
let mut registry: AgentRegistry<String, String> = AgentRegistry::new();
registry.register(Arc::new(SeniorCoder));
registry.register(Arc::new(HumanReviewer));

let outcome = run_with_escalation(&Coder, spec, &registry, 3).await;
// outcome.context.delegation_chain shows every agent that touched the task

// run one agent concurrently over many inputs
let outcomes = join_all(&Researcher, topics).await;

// run several different agents concurrently, pick a winner by confidence
let candidates: Vec<(String, Arc<dyn Agent<Input = _, Output = _>>)> = vec![
    ("aggressive".into(), Arc::new(AggressivePlanner)),
    ("conservative".into(), Arc::new(ConservativePlanner)),
];
let trace = speculate(candidates, task).await;
// losing candidates are recorded as rejected Branches on a "speculate" step
```

`join_all` and `speculate` currently run concurrently on the same task (via `futures::future::join_all`) rather than across OS threads, and there's no shared step-budget spanning concurrent branches yet — both are tracked as open follow-ups rather than silently assumed.

---

## branching

```
// value branching — every arm recorded in the trace
branch doc.word_count {
    0       => reject("empty document"),
    1..100  => emit(Category::Short),
    _       => emit(Category::Long),
}

// confidence branching — route on certainty
branch confidence(score) {
    0.9..1.0 => emit(Decision::Proceed),
    0.6..0.9 => { observe gather_more_context(ctx); emit(Decision::ProceedWithCaution) },
    _        => delegate(HumanReviewer, ctx),
}

// speculative — run multiple branches concurrently, pick the winner
speculate {
    A: aggressive_plan(task),
    B: conservative_plan(task),
    C: creative_plan(task),
}
pick_best(|plans| plans.max_by(|p| p.confidence))
```

---

## design principles

**Ownership of reasoning.** A trace has a single owner. Merging or diffing traces is explicit — no hidden shared state between agents.

**No silent failures.** `reject()` is a first-class operation. Every abandoned branch is recorded with a reason. `TraceErr` is a named enum; there are no mystery panics.

**Serialization guarantee.** Any type that participates in task management must be serializable. There is no runtime surprise where a pipeline cannot be checkpointed.

**Provider-agnostic.** `tool` declarations are typed interfaces. Providers (LLMs, APIs, storage) are swappable without changing agent logic — the same pattern as Rust trait objects.

---

## related work

- `agent_trace` — Joe's personal traced agentic execution crate (the design inspiration for `Trace<T>`)
- `doob` — agent-first task CLI that `trace-task` could back
- `orca-strait` — parallel TDD sub-agent orchestrator that could consume `TaskRegistry`
- LangGraph, CrewAI — graph-based agentic frameworks; `trace::` differs by making provenance first-class at the type level rather than at the runtime level

---

## status

Early design + reference implementation. `trace-core` and `trace-task` compile and are usable as Rust libraries today. The `trace::` surface syntax and compiler are design artifacts — contributions and discussion welcome.

## license

MIT OR Apache-2.0
