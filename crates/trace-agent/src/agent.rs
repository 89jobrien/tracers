use crate::context::AgentContext;
use crate::hooks::EscalationAction;
use async_trait::async_trait;
use serde::Serialize;
use trace_core::Trace;

/// The unit of computation in trace::.
///
/// An `Agent` declares intent (`goal`), an optional resource limit
/// (`budget`), and an optional certainty requirement
/// (`confidence_threshold`), then implements `run()` to produce a
/// `Trace<Output>`.
///
/// Lifecycle hooks (`on_low_confidence`, `on_budget_exceeded`,
/// `on_step_failure`) are declarative: they return an
/// [`EscalationAction`] rather than performing the escalation
/// themselves. `spawn()` evaluates the resulting trace against these
/// hooks after `run()` completes.
#[async_trait]
pub trait Agent: Send + Sync {
    /// The type consumed by this agent's `run` step.
    type Input: Send;
    /// The type produced on success, wrapped in `Trace<Output>`.
    type Output: Clone + Serialize + Send;

    /// A short, stable identifier used in delegation chains and traces.
    fn name(&self) -> &str;

    /// A human-readable statement of what this agent is trying to achieve.
    /// Runtime-queryable — not just a comment.
    fn goal(&self) -> &str;

    /// Minimum acceptable confidence for a step before
    /// `on_low_confidence` is consulted. Defaults to `0.7`.
    fn confidence_threshold(&self) -> f64 {
        0.7
    }

    /// Maximum number of steps this agent may take. `None` means
    /// unbounded. Defaults to `None`.
    fn budget(&self) -> Option<usize> {
        None
    }

    /// Run the agent's logic, producing a `Trace<Output>`.
    ///
    /// Implementations should call [`AgentContext::record_step`] for
    /// every unit of work so budget enforcement stays accurate.
    async fn run(&self, input: Self::Input, ctx: &mut AgentContext) -> Trace<Self::Output>;

    /// Escalation to apply when any step's confidence falls below
    /// [`Agent::confidence_threshold`]. Defaults to no escalation.
    fn on_low_confidence(&self) -> EscalationAction {
        EscalationAction::None
    }

    /// Escalation to apply when the agent exceeds its declared budget.
    /// Defaults to no escalation.
    fn on_budget_exceeded(&self) -> EscalationAction {
        EscalationAction::None
    }

    /// Escalation to apply when a step fails outright. Defaults to no
    /// escalation.
    fn on_step_failure(&self) -> EscalationAction {
        EscalationAction::None
    }
}
