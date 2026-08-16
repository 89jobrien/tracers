//! Integration test: `assert_trace!` against the real
//! `Guesser -> Careful -> Expert` escalation chain from
//! `tracers_runtime::fixtures` — the same flow proven end-to-end by
//! `crates/runtime/tests/escalation_wiring.rs`, now also asserted on
//! shape (not just final value) via `trace-test`.

use std::sync::Arc;
use tracers_runtime::fixtures::{Careful, Expert, Guesser};
use tracers_runtime::{AgentRegistry, run_with_escalation};
use tracers_trace_test::assert_trace;

#[tokio::test]
async fn full_escalation_chain_has_expected_shape() {
    let mut registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
    registry.register(Arc::new(Careful));
    registry.register(Arc::new(Expert));

    let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;

    assert_trace!(&outcome, {
        contains_step("verify");
        confidence_below("verify", 1.0);
        escalates_to("Careful");
        escalates_to("Expert");
        never_step("publish");
    });
}

#[tokio::test]
#[should_panic(expected = "expected escalation to")]
async fn escalates_to_fails_for_an_agent_never_reached() {
    let registry: AgentRegistry<(), &'static str> = AgentRegistry::new();
    let outcome = run_with_escalation(&Guesser, (), &registry, 5).await;

    assert_trace!(&outcome, {
        escalates_to("NeverRegistered");
    });
}
