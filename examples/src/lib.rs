//! Shared scaffolding for the runnable `trace::` examples.
//!
//! Each example in this directory is standalone and printable — run one with
//! `cargo run -p trace-lang-examples --example <name>`. What lives here is
//! only what more than one of them needs: output formatting, a scratch path
//! for checkpoint files, and the two-agent draft/edit pair that both the
//! escalation and lineage examples build on.

use std::path::PathBuf;

use async_trait::async_trait;
use trace_lang_agent::{Agent, AgentContext, EscalationAction};
use trace_lang_core::{Step, StepCost, Trace};

/// Print a section header, so example output reads as a walkthrough rather
/// than a wall of `println!`s.
pub fn heading(title: &str) {
    println!(
        "\n── {title} {}",
        "─".repeat(66_usize.saturating_sub(title.len()))
    );
}

/// Print one labelled fact.
pub fn fact(label: &str, value: impl std::fmt::Display) {
    println!("  {label:<28} {value}");
}

/// Render a trace's causal chain the way `trace-cli show` does.
pub fn print_chain<T: Clone + serde::Serialize>(trace: &Trace<T>) {
    for (i, step) in trace.causal_chain().iter().enumerate() {
        let confidence = step
            .confidence
            .map(|c| format!("{c:.2}"))
            .unwrap_or_else(|| "—".to_string());
        let cost = step
            .cost
            .map(|c| c.to_string())
            .unwrap_or_else(|| "—".to_string());
        println!(
            "  {}. {:<22} confidence {:<6} {:?} · {}",
            i + 1,
            step.name,
            confidence,
            step.outcome,
            cost
        );
    }
}

/// A unique scratch path for a checkpoint file, so two examples running at
/// once never fight over the same file.
pub fn scratch_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "trace-lang-example-{name}-{}.json",
        uuid::Uuid::new_v4()
    ))
}

/// Produces a first draft, and knows it isn't sure enough to ship: its
/// confidence lands below the default threshold, so `spawn` consults
/// `on_low_confidence` and it escalates to `Editor`.
pub struct Drafter;

#[async_trait]
impl Agent for Drafter {
    type Input = String;
    type Output = String;

    fn name(&self) -> &str {
        "Drafter"
    }

    fn goal(&self) -> &str {
        "produce a first-pass summary, escalating when unsure"
    }

    async fn run(&self, input: Self::Input, ctx: &mut AgentContext) -> Trace<Self::Output> {
        let mut trace = Trace::new(format!("draft summary of {input:?}"));
        if let Err(err) = ctx.record_step() {
            return Trace::failed(err);
        }
        trace.push_step(
            Step::named("skim-source")
                .with_confidence(0.55)
                .with_cost(StepCost::new(1_200, 180).with_dollars(0.0041))
                .with_note("only skimmed — the source is longer than the context window"),
        );
        trace
    }

    fn on_low_confidence(&self) -> EscalationAction {
        EscalationAction::Delegate("Editor".to_string())
    }
}

/// Takes the same input the `Drafter` failed to be sure about and does the
/// work properly. Registered under its own name so the runtime can resolve
/// `Delegate("Editor")` to it.
pub struct Editor;

#[async_trait]
impl Agent for Editor {
    type Input = String;
    type Output = String;

    fn name(&self) -> &str {
        "Editor"
    }

    fn goal(&self) -> &str {
        "read the source properly and produce a summary worth shipping"
    }

    async fn run(&self, input: Self::Input, ctx: &mut AgentContext) -> Trace<Self::Output> {
        let mut trace = Trace::new(format!("edited summary of {input:?}"));
        if let Err(err) = ctx.record_step() {
            return Trace::failed(err);
        }
        trace.push_step(
            Step::named("read-in-full")
                .with_confidence(0.94)
                .with_cost(StepCost::new(18_400, 620).with_dollars(0.0612)),
        );
        trace
    }
}
