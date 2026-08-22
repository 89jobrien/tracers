//! `Trace<T>` end to end: build one, then interrogate it.
//!
//! The point of `trace::` is that everything below is a query against a
//! *value*, not a scrape of a log file. Run it:
//!
//! ```text
//! cargo run -p trace-lang-examples --example trace_basics
//! ```

use std::time::Duration;

use trace_lang_core::{Branch, Span, Step, StepCost, Trace, TraceErr};
use trace_lang_examples::{fact, heading, print_chain};

fn main() {
    // A trace starts as a value with a provenance chain of zero steps.
    let mut trace = Trace::new("Kubernetes is the right call here".to_string());

    // Each unit of reasoning appends a Step. `Span` times a region without
    // anyone having to remember to read a clock twice.
    let span = Span::start("gather-constraints");
    let constraints = ["team of 4", "3 services", "spiky traffic"];
    trace.push_step(
        Step::named("gather-constraints")
            .with_confidence(0.98)
            .with_duration(span.finish())
            .with_cost(StepCost::new(800, 120).with_dollars(0.0028))
            .with_note(format!("{} constraints collected", constraints.len())),
    );

    trace.push_step(
        Step::named("choose-orchestrator")
            .with_confidence(0.71)
            .with_duration(Duration::from_millis(2_400))
            .with_cost(StepCost::new(9_200, 1_450).with_dollars(0.0312)),
    );
    // A step the agent tried and abandoned. It stays in the chain — "why not
    // Nomad" is a question the trace can still answer months later.
    trace.push_step(
        Step::named("evaluate-nomad")
            .with_confidence(0.40)
            .with_duration(Duration::from_millis(900))
            .with_cost(StepCost::new(3_100, 210).with_dollars(0.0104))
            .rejected("no team experience, and no managed offering on our cloud"),
    );

    trace.push_step(
        Step::named("write-recommendation")
            .with_confidence(0.88)
            .with_duration(Duration::from_millis(300))
            .with_cost(StepCost::new(1_100, 2_300).with_dollars(0.0389)),
    );

    heading("causal chain");
    print_chain(&trace);

    heading("where the time went");
    for step in trace.bottlenecks().iter().take(2) {
        fact(
            &step.name,
            format!("{:?}", step.duration.unwrap_or_default()),
        );
    }

    heading("where the money went");
    let total = trace.total_cost();
    fact("total", total);
    fact("total tokens", total.total_tokens());
    for step in trace.priciest_steps().iter().take(2) {
        fact(&step.name, step.cost.unwrap_or_default());
    }

    heading("where the doubt is");
    for step in trace.low_confidence() {
        fact(
            &step.name,
            format!("confidence {:.2}", step.confidence.unwrap_or_default()),
        );
    }

    heading("what was considered and dropped");
    for step in trace.rejected_branches() {
        fact(&step.name, format!("{:?}", step.outcome));
    }

    heading("speculation recorded as branches");
    // `speculate` in trace-lang-runtime produces this shape automatically;
    // building it by hand shows what it is underneath.
    let mut decision = Step::named("pick-database").with_confidence(0.82);
    decision.branches = vec![
        Branch::taken("postgres").with_confidence(0.82),
        Branch::rejected("dynamodb", "relational access patterns").with_confidence(0.44),
        Branch::rejected("sqlite", "needs multi-writer").with_confidence(0.20),
    ];
    trace.push_step(decision);
    for branch in trace.all_branches() {
        fact(&branch.label, format!("{:?}", branch.outcome));
    }

    heading("the value, and its provenance pointer");
    fact("value", trace.value().cloned().unwrap_or_default());
    fact("trace_ref", trace.trace_ref());
    fact("steps", trace.causal_chain().len());

    heading("failure propagates like Result");
    // `Trace<T>` converts into `Result<T, TraceErr>`, so `?` works exactly as
    // it does anywhere else — the trace is the thing that explains the error.
    let failed: Trace<String> = Trace::failed(TraceErr::tool_failed("kubectl", "context not set"));
    let as_result: Result<String, TraceErr> = failed.into();
    match as_result {
        Ok(value) => fact("unexpectedly ok", value),
        Err(err) => fact("error", err),
    }
}
