# trace-lang-agent

[![crates.io](https://img.shields.io/crates/v/trace-lang-agent.svg)](https://crates.io/crates/trace-lang-agent)
[![docs.rs](https://docs.rs/trace-lang-agent/badge.svg)](https://docs.rs/trace-lang-agent)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://github.com/89jobrien/trace-lang)

The `Agent` trait, `spawn`/`delegate`, and lifecycle escalation hooks for
`trace::` pipelines.

An `Agent` is the unit of computation in `trace::` — it declares a goal, an
optional step budget, and an optional confidence threshold, then implements
`run()` to produce a `Trace<Output>` (from `trace-lang-core`). `spawn()` launches
an agent and evaluates its lifecycle hooks against the resulting trace: budget
exhaustion, step failure, and low confidence each have a declarative escalation
path rather than being handled ad hoc at the call site. `delegate()` transfers
execution to another agent while preserving the delegation chain.

## Install

```toml
[dependencies]
trace-lang-agent = { path = "../agent" }
async-trait   = "0.1"
```

## Quick start

```rust
use async_trait::async_trait;
use trace_lang_agent::{Agent, AgentContext, spawn};
use trace_lang_core::{Trace, Step};

struct Greeter;

#[async_trait]
impl Agent for Greeter {
    type Input = String;
    type Output = String;

    fn name(&self) -> &str { "Greeter" }
    fn goal(&self) -> &str { "produce a greeting for the user" }

    async fn run(&self, input: Self::Input, ctx: &mut AgentContext) -> Trace<Self::Output> {
        ctx.record_step().expect("first step never exceeds budget");
        let mut trace = Trace::new(format!("hello, {input}!"));
        trace.push_step(Step::named("greet").with_confidence(0.97));
        trace
    }
}

async fn example() {
    let outcome = spawn(&Greeter, "world".to_string()).await;
    assert_eq!(outcome.trace.value(), Some(&"hello, world!".to_string()));
}
```

## The `Agent` trait

```rust
#[async_trait]
pub trait Agent: Send + Sync {
    type Input: Send;
    type Output: Clone + Serialize + Send;

    fn name(&self) -> &str;
    fn goal(&self) -> &str;

    fn confidence_threshold(&self) -> f64 { 0.7 }   // default
    fn budget(&self) -> Option<usize> { None }       // default: unbounded

    async fn run(&self, input: Self::Input, ctx: &mut AgentContext) -> Trace<Self::Output>;

    fn on_low_confidence(&self) -> EscalationAction { EscalationAction::None }
    fn on_budget_exceeded(&self) -> EscalationAction { EscalationAction::None }
    fn on_step_failure(&self) -> EscalationAction { EscalationAction::None }
}
```

Lifecycle hooks are declarative: they return an `EscalationAction` describing
*what should happen*, not perform it themselves. `spawn`/`delegate` evaluate the
resulting trace against these hooks after `run()` completes; resolving the action
(e.g. actually invoking the named delegate) is the caller's — typically
`trace-lang-runtime`'s — job.

Implementations must call `AgentContext::record_step()` for every unit of work so
budget enforcement stays accurate.

## `AgentContext`

Per-run state threaded through `Agent::run`.

| field / method | purpose |
| --- | --- |
| `agent_name`, `steps_taken`, `budget` | current run's identity and progress |
| `delegation_chain: Vec<String>` | ordered agent names that handed off execution to reach this point |
| `record_step()` | increments `steps_taken`; returns `Err(TraceErr::BudgetExhausted)` once it would exceed `budget` |
| `budget_remaining()` | `Option<usize>`, `None` if unbounded |
| `is_budget_exhausted()` | `true` once `steps_taken >= budget` |

`spawn()` starts a fresh context (`delegation_chain = [agent_name]`); `delegate()`
extends the caller's chain instead of replacing it, so a multi-agent handoff is
always reconstructable from `delegation_chain` alone.

## `EscalationAction`

```rust
enum EscalationAction {
    None,                             // proceed as normal
    Delegate(String),                 // hand off to another agent, named
    Emit(TraceErr),                   // abort with this error rather than escalating further
    RequireApproval(ApprovalRequest), // stop and wait for a person
}
```

Helpers: `is_none()`, `delegate_target() -> Option<&str>`,
`approval_request() -> Option<&ApprovalRequest>`, and `needs_a_human()` — true for
`RequireApproval` and `Emit`, the two actions no registry can discharge.

`RequireApproval` is not a delegation to a "HumanReviewer" agent; that already
works through `Delegate` and is fine when "human" is really another named endpoint.
This is a genuine stop-the-world pause: the caller parks the work (typically as
`TaskStatus::Paused` in a checkpointed `TaskRegistry`) and resumes it when a
decision arrives through some external channel, possibly days later.

## `spawn` and `delegate`

```rust
pub async fn spawn<A: Agent + ?Sized>(agent: &A, input: A::Input) -> SpawnOutcome<A::Output>;

pub async fn delegate<A: Agent + ?Sized>(
    agent: &A,
    input: A::Input,
    from: &AgentContext,
) -> SpawnOutcome<A::Output>;
```

Both accept `A: ?Sized`, so they work with `&dyn Agent<Input = I, Output = O>`
trait objects as well as concrete sized agent types — this is what lets
`trace-lang-runtime`'s `AgentRegistry` call them without duplicating logic between
the sized and trait-object paths.

`SpawnOutcome<O>` bundles the produced `Trace<O>`, the `AgentContext` it ran
under, and the `EscalationAction` a lifecycle hook recommended (if any):

```rust
pub struct SpawnOutcome<O> {
    pub trace: Trace<O>,
    pub context: AgentContext,
    pub escalation: EscalationAction,
}
```

Hook evaluation logic (shared between `spawn` and `delegate`): a
`BudgetExhausted` error consults `on_budget_exceeded`; any other error consults
`on_step_failure`; a successful trace with any step below `confidence_threshold`
consults `on_low_confidence`; otherwise `EscalationAction::None`.

One thing evaluation does on its own: a hook's signature is
`fn(&self) -> EscalationAction`, so a hook raising an `ApprovalRequest` cannot see
the trace it is reacting to. Build it with `ApprovalRequest::unattached(question)`
and `spawn`/`delegate` stamp the real `TraceRef` on the way out, so a caller never
receives a question about a run it cannot look up.

`spawn` and `delegate` do not act on the returned escalation — delegation is not
performed automatically. `trace-lang-runtime::run_with_escalation` is the layer that
resolves `Delegate(name)` against a live agent registry and re-runs.

## `Contract<I, O>`

Design-by-contract for a single step. A step can be *technically* successful — no
`TraceErr`, no panic — and still substantively wrong: an empty summary, a malformed
identifier, a number outside its range.

```rust
use trace_lang_agent::{Contract, contract_step};

let contract: Contract<String, Summary> = Contract::new()
    .pre(|source: &String| {
        if source.trim().is_empty() { Err("nothing to summarize".into()) } else { Ok(()) }
    })
    .post(|summary: &Summary| {
        if summary.text.is_empty() { Err("summary must not be empty".into()) } else { Ok(()) }
    });

let checked = contract.check_post(&output);
trace.push_step(contract_step("summary-contract", &checked));
```

`check_pre`/`check_post` return `Result<(), TraceErr>`, with a violation becoming
`TraceErr::ContractViolated { message }` naming the invariant that broke
(`"postcondition: summary must not be empty"`). That is a distinct variant from
`ToolFailed` on purpose: `on_step_failure` should treat "the tool broke" (retry) and
"the tool worked and returned something forbidden" (escalate to someone who
understands the domain) differently.

`contract_step(name, &outcome)` turns a check into the `Step` that records it —
`Taken` when the invariant held, `Failed { message }` when it did not. A satisfied
contract is still worth a step: it is evidence the invariant held at that point in
the chain.

Checking is explicit at the call site inside `Agent::run`, so an agent in a hot loop
simply doesn't call it — there is no global toggle to keep in sync, and no per-agent
opt-in flag to declare.

## Testing

```bash
cargo test -p trace-lang-agent
```

`AgentContext` budget tracking is covered by `proptest` property tests and a
`kani` proof (`budget_remaining_and_is_exhausted_agree`) that `budget_remaining`'s
saturating subtraction never disagrees with `is_budget_exhausted`, run under
`cargo kani` rather than `cargo test`.
