//! Step pre/post-conditions: catching output that is technically successful
//! and substantively wrong.
//!
//! ```text
//! cargo run -p trace-lang-examples --example contracts
//! ```

use async_trait::async_trait;
use trace_lang_agent::{Agent, AgentContext, Contract, EscalationAction, contract_step, spawn};
use trace_lang_core::{Step, Trace, TraceErr};
use trace_lang_examples::{fact, heading, print_chain};

/// A summarizer that declares what a valid summary looks like, then holds
/// itself to it. Both failure modes below are "successful" runs by every
/// other measure: no panic, no tool error, a `String` came back.
struct Summarizer {
    /// Simulates the model returning something useless.
    misbehave: bool,
}

impl Summarizer {
    /// The invariants a summary must satisfy, declared next to the step
    /// logic they constrain rather than in a validation layer elsewhere.
    fn contract() -> Contract<String, String> {
        Contract::new()
            .pre(|source: &String| {
                if source.trim().is_empty() {
                    Err("nothing to summarize".to_string())
                } else {
                    Ok(())
                }
            })
            .post(|summary: &String| {
                if summary.trim().is_empty() {
                    Err("summary must not be empty".to_string())
                } else if summary.len() > 80 {
                    Err(format!(
                        "summary must be under 80 chars, got {}",
                        summary.len()
                    ))
                } else {
                    Ok(())
                }
            })
    }
}

#[async_trait]
impl Agent for Summarizer {
    type Input = String;
    type Output = String;

    fn name(&self) -> &str {
        "Summarizer"
    }

    fn goal(&self) -> &str {
        "produce a short summary that is actually a summary"
    }

    async fn run(&self, input: Self::Input, ctx: &mut AgentContext) -> Trace<Self::Output> {
        let contract = Self::contract();
        let mut trace = Trace::new(String::new());

        // Precondition: check before spending anything on the call.
        let pre = contract.check_pre(&input);
        trace.push_step(contract_step("precondition", &pre));
        if let Err(err) = pre {
            return with_chain(Trace::failed(err), trace);
        }

        if let Err(err) = ctx.record_step() {
            return with_chain(Trace::failed(err), trace);
        }

        let summary = if self.misbehave {
            String::new() // the model returned nothing useful
        } else {
            format!("{} words on {input}", input.len())
        };
        trace.push_step(Step::named("summarize").with_confidence(0.9));

        // Postcondition: the step "succeeded", but did it produce something
        // downstream code can actually use?
        let post = contract.check_post(&summary);
        trace.push_step(contract_step("postcondition", &post));
        if let Err(err) = post {
            return with_chain(Trace::failed(err), trace);
        }

        with_chain(Trace::new(summary), trace)
    }

    fn on_step_failure(&self) -> EscalationAction {
        // A contract violation and a broken tool deserve different answers:
        // retrying a tool can work, retrying a violated invariant usually
        // just violates it again.
        EscalationAction::Delegate("HumanEditor".to_string())
    }
}

/// Move the steps accumulated so far onto whichever trace is being returned.
fn with_chain(result: Trace<String>, chain: Trace<String>) -> Trace<String> {
    Trace::merge(result, chain)
}

#[tokio::main]
async fn main() {
    heading("contract satisfied");
    let good = spawn(&Summarizer { misbehave: false }, "RFC-9110".to_string()).await;
    print_chain(&good.trace);
    fact("value", good.trace.value().cloned().unwrap_or_default());
    fact("escalation", format!("{:?}", good.escalation));

    heading("postcondition violated");
    // The run did not error. It returned a `String`. It is still wrong, and
    // the trace says exactly which invariant it broke.
    let bad = spawn(&Summarizer { misbehave: true }, "RFC-9110".to_string()).await;
    print_chain(&bad.trace);
    fact("error", format!("{:?}", bad.trace.error()));
    fact("escalation", format!("{:?}", bad.escalation));

    heading("precondition violated");
    let blank = spawn(&Summarizer { misbehave: false }, "   ".to_string()).await;
    print_chain(&blank.trace);
    fact("error", format!("{:?}", blank.trace.error()));

    heading("why a dedicated error variant matters");
    // `on_step_failure` fires for every error. Matching on the variant is
    // what lets a hook tell "the tool broke" from "the tool worked and
    // returned something forbidden".
    for trace in [&bad.trace, &blank.trace] {
        let verdict = match trace.error() {
            Some(TraceErr::ContractViolated { message }) => {
                format!("invariant broken — {message}")
            }
            Some(TraceErr::ToolFailed { tool, .. }) => format!("retry {tool}"),
            Some(other) => format!("other failure: {other}"),
            None => "no failure".to_string(),
        };
        fact("verdict", verdict);
    }
}
