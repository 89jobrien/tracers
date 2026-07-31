//! `tracers-agent` — the `Agent` trait, `spawn`/`delegate`, and lifecycle
//! escalation hooks for trace:: pipelines.
//!
//! An `Agent` is the unit of computation in trace:: — it declares a goal,
//! an optional step budget, and an optional confidence threshold, then
//! implements `run()` to produce a `Trace<Output>`.
//!
//! `spawn()` launches an agent and evaluates its lifecycle hooks against
//! the resulting trace: budget exhaustion, step failure, and low
//! confidence each have a declarative escalation path rather than being
//! handled ad hoc at the call site.
//!
//! `delegate()` transfers execution to another agent while preserving the
//! delegation chain, so `AgentContext::delegation_chain` always shows the
//! full handoff history — exactly what `Trace::causal_chain()` needs to
//! explain a multi-agent run.
//!
//! # Example
//!
//! ```rust
//! use async_trait::async_trait;
//! use tracers_agent::{Agent, AgentContext, spawn};
//! use tracers_core::{Trace, Step};
//!
//! struct Greeter;
//!
//! #[async_trait]
//! impl Agent for Greeter {
//!     type Input = String;
//!     type Output = String;
//!
//!     fn name(&self) -> &str { "Greeter" }
//!     fn goal(&self) -> &str { "produce a greeting for the user" }
//!
//!     async fn run(&self, input: Self::Input, ctx: &mut AgentContext) -> Trace<Self::Output> {
//!         ctx.record_step().expect("first step never exceeds budget");
//!         let mut trace = Trace::new(format!("hello, {input}!"));
//!         trace.push_step(Step::named("greet").with_confidence(0.97));
//!         trace
//!     }
//! }
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let outcome = spawn(&Greeter, "world".to_string()).await;
//! assert_eq!(outcome.trace.value(), Some(&"hello, world!".to_string()));
//! # });
//! ```

pub mod agent;
pub mod context;
pub mod hooks;
pub mod spawn;

pub use agent::Agent;
pub use context::AgentContext;
pub use hooks::EscalationAction;
pub use spawn::{SpawnOutcome, delegate, spawn};
