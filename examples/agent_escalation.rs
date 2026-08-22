//! Agents, lifecycle hooks, and delegation resolved through a registry.
//!
//! `Drafter` isn't confident enough in its own output, so it escalates.
//! Nothing in `Drafter` knows who `Editor` is — it names a target, and the
//! runtime resolves that name against `AgentRegistry`.
//!
//! ```text
//! cargo run -p trace-lang-examples --example agent_escalation
//! ```

use std::sync::Arc;

use trace_lang_agent::spawn;
use trace_lang_examples::{Drafter, Editor, fact, heading, print_chain};
use trace_lang_runtime::{AgentRegistry, join_all, run_with_escalation, speculate};

#[tokio::main]
async fn main() {
    let source = "RFC-9110".to_string();

    heading("spawn: run one agent, and see what it recommends");
    // `spawn` runs the agent and evaluates its hooks — it does *not* act on
    // the escalation. The decision stays explicit at the call site.
    let outcome = spawn(&Drafter, source.clone()).await;
    print_chain(&outcome.trace);
    fact("steps taken", outcome.context.steps_taken);
    fact("escalation", format!("{:?}", outcome.escalation));
    fact(
        "recommended target",
        outcome.escalation.delegate_target().unwrap_or("none"),
    );

    heading("run_with_escalation: resolve that recommendation");
    let mut registry: AgentRegistry<String, String> = AgentRegistry::new();
    registry.register(Arc::new(Editor));
    fact("registered agents", registry.len());

    let resolved = run_with_escalation(&Drafter, source.clone(), &registry, 3).await;
    fact("value", resolved.trace.value().cloned().unwrap_or_default());
    fact(
        "delegation chain",
        resolved.context.delegation_chain.join(" → "),
    );
    fact("unresolved", format!("{:?}", resolved.unresolved));
    fact("cost of the winning run", resolved.trace.total_cost());

    heading("an unregistered target stops the loop honestly");
    // Same `Drafter`, empty registry: the escalation comes back unresolved
    // rather than being silently swallowed.
    let empty: AgentRegistry<String, String> = AgentRegistry::new();
    let stuck = run_with_escalation(&Drafter, source.clone(), &empty, 3).await;
    fact("unresolved", format!("{:?}", stuck.unresolved));

    heading("join_all: fan out over many inputs");
    let inputs = vec!["RFC-9110".to_string(), "RFC-6455".to_string()];
    for outcome in join_all(&Editor, inputs).await {
        fact(
            "produced",
            outcome
                .trace
                .value()
                .cloned()
                .unwrap_or_else(|| "—".to_string()),
        );
    }

    heading("speculate: race two agents, keep the confident one");
    // Both agents answer the same question; the winner is chosen by mean
    // step confidence, and the loser is recorded as a rejected `Branch`
    // rather than thrown away.
    let candidates: Vec<(
        String,
        Arc<dyn trace_lang_agent::Agent<Input = String, Output = String>>,
    )> = vec![
        ("Drafter".to_string(), Arc::new(Drafter)),
        ("Editor".to_string(), Arc::new(Editor)),
    ];
    let winner = speculate(candidates, source).await;
    fact("winner", winner.value().cloned().unwrap_or_default());
    for branch in winner.all_branches() {
        fact(
            &branch.label,
            format!(
                "{:?} (confidence {:.2})",
                branch.outcome,
                branch.confidence.unwrap_or_default()
            ),
        );
    }
}
