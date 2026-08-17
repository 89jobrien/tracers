//! Real agent fixtures shared between this crate's own integration tests
//! and downstream crates' tests (via the `test-support` feature) — a
//! `Guesser -> Careful -> Expert` chain exercising a low-confidence
//! escalation followed by a budget-exhaustion escalation before a third
//! agent finally succeeds. Moved out of `tests/escalation_wiring.rs` so
//! `trace-test`'s integration test can drive the same proven flow instead
//! of a fixture written to make its own macro look good.

use async_trait::async_trait;
use trace_lang_agent::{Agent, AgentContext, EscalationAction};
use trace_lang_core::{Step, Trace};

/// Always produces a low-confidence step, escalating to "Careful".
pub struct Guesser;

#[async_trait]
impl Agent for Guesser {
    type Input = ();
    type Output = &'static str;

    fn name(&self) -> &str {
        "Guesser"
    }
    fn goal(&self) -> &str {
        "produce a low-confidence first guess"
    }
    fn confidence_threshold(&self) -> f64 {
        0.9
    }

    async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
        ctx.record_step().unwrap();
        let mut t = Trace::new("shaky guess");
        t.push_step(Step::named("guess").with_confidence(0.2));
        t
    }

    fn on_low_confidence(&self) -> EscalationAction {
        EscalationAction::Delegate("Careful".to_string())
    }
}

/// Exhausts its one-step budget immediately, escalating to "Expert".
pub struct Careful;

#[async_trait]
impl Agent for Careful {
    type Input = ();
    type Output = &'static str;

    fn name(&self) -> &str {
        "Careful"
    }
    fn goal(&self) -> &str {
        "run out of budget while double-checking"
    }
    fn budget(&self) -> Option<usize> {
        Some(1)
    }

    async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
        ctx.record_step().unwrap();
        if let Err(e) = ctx.record_step() {
            return Trace::failed(e);
        }
        Trace::new("careful answer")
    }

    fn on_budget_exceeded(&self) -> EscalationAction {
        EscalationAction::Delegate("Expert".to_string())
    }
}

/// Succeeds cleanly with no further escalation.
pub struct Expert;

#[async_trait]
impl Agent for Expert {
    type Input = ();
    type Output = &'static str;

    fn name(&self) -> &str {
        "Expert"
    }
    fn goal(&self) -> &str {
        "settle the task with high confidence"
    }

    async fn run(&self, _input: (), ctx: &mut AgentContext) -> Trace<&'static str> {
        ctx.record_step().unwrap();
        let mut t = Trace::new("expert answer");
        t.push_step(Step::named("verify").with_confidence(0.95));
        t
    }
}
