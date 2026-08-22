//! A pipeline that stops and waits for a person.
//!
//! The interesting part is what this example does *not* need: no approval
//! queue, no side table, no bespoke resume path. `TaskRegistry` already
//! checkpoints every transition, so "paused waiting on a human" is just
//! another state it persists.
//!
//! ```text
//! cargo run -p trace-lang-examples --example human_in_the_loop
//! ```

use async_trait::async_trait;
use trace_lang_agent::{Agent, AgentContext, EscalationAction, spawn};
use trace_lang_core::{ApprovalDecision, ApprovalRequest, Step, Trace, TraceErr};
use trace_lang_examples::{fact, heading, print_chain, scratch_path};
use trace_lang_task::{FileCheckpointStore, Priority, Task, TaskRegistry};

/// Issues refunds, but refuses to issue a large one on its own authority.
struct Refunder {
    limit_usd: u64,
}

#[async_trait]
impl Agent for Refunder {
    type Input = u64;
    type Output = String;

    fn name(&self) -> &str {
        "Refunder"
    }

    fn goal(&self) -> &str {
        "issue a refund, escalating to a human above the auto-approval limit"
    }

    async fn run(&self, amount_usd: u64, ctx: &mut AgentContext) -> Trace<Self::Output> {
        if let Err(err) = ctx.record_step() {
            return Trace::failed(err);
        }

        if amount_usd > self.limit_usd {
            let mut trace: Trace<String> =
                Trace::failed(TraceErr::other("above the auto-approval limit"));
            trace.push_step(
                Step::named("check-limit")
                    .with_confidence(0.99)
                    .with_note(format!(
                        "${amount_usd} exceeds the ${} limit",
                        self.limit_usd
                    )),
            );
            return trace;
        }

        let mut trace = Trace::new(format!("refunded ${amount_usd}"));
        trace.push_step(Step::named("issue-refund").with_confidence(0.97));
        trace
    }

    fn on_step_failure(&self) -> EscalationAction {
        // Not a `Delegate`: no agent anywhere can discharge this — the work
        // stops until a person answers. The hook cannot see the trace it is
        // reacting to, so `spawn` stamps the request with it on the way out.
        EscalationAction::RequireApproval(ApprovalRequest::unattached(
            "approve a refund above the automatic limit?",
        ))
    }
}

#[tokio::main]
async fn main() -> Result<(), TraceErr> {
    let path = scratch_path("human-in-the-loop");
    let store = FileCheckpointStore::new(&path);
    let agent = Refunder { limit_usd: 500 };

    heading("a refund inside the limit just happens");
    let small = spawn(&agent, 42).await;
    print_chain(&small.trace);
    fact("value", small.trace.value().cloned().unwrap_or_default());
    fact("escalation", format!("{:?}", small.escalation));

    heading("a refund above the limit stops");
    let large = spawn(&agent, 4_000).await;
    print_chain(&large.trace);
    let request = large
        .escalation
        .approval_request()
        .expect("the agent asked for approval")
        .clone()
        // Add the numbers a human needs in order to decide.
        .with_context(serde_json::json!({ "amount_usd": 4_000, "customer": "acme-corp" }));
    fact("question", &request.question);
    fact("about trace", &request.trace);
    assert_eq!(request.trace, large.trace.trace_ref());

    heading("park the work in the task graph");
    let refund = Task::new("refund acme-corp").with_priority(Priority::Critical);
    let refund_id = refund.id;
    let notify = Task::new("email acme-corp").depends_on(refund_id);
    let notify_id = notify.id;
    let mut registry = TaskRegistry::from(vec![refund, notify]);
    registry
        .get_mut(refund_id)
        .expect("the refund task exists")
        .assign_to(agent.name());
    registry.pause(refund_id, request, &store)?;
    fact("paused", registry.paused().len());
    fact("ready to run", registry.ready_tasks().len());

    heading("the process exits; the question does not");
    // Everything above is on disk. Nothing is holding a future open, no
    // thread is blocked, and the decision can take days.
    drop(registry);
    let mut inbox = TaskRegistry::load(&store)?;
    let waiting = inbox.paused();
    for task in &waiting {
        let request = task
            .approval_request()
            .expect("a paused task has a request");
        fact("waiting on", &request.question);
        fact("context", &request.context);
        fact("assigned to", task.assigned_to.as_deref().unwrap_or("—"));
        fact("waiting for", format!("{}s", request.age().num_seconds()));
    }

    heading("a person answers");
    inbox.resume(
        refund_id,
        ApprovalDecision::approve_with_note("joe", "checked with finance"),
        &store,
    )?;
    fact("paused", inbox.paused().len());
    fact("ready to run", inbox.ready_tasks().len());

    // Approved work is ordinary work again: run it, complete it, and the
    // dependent task unblocks exactly as it would have without the detour.
    let rerun = spawn(&Refunder { limit_usd: 10_000 }, 4_000).await;
    inbox.complete(refund_id, rerun.trace.trace_ref(), &store)?;
    fact("refund", rerun.trace.value().cloned().unwrap_or_default());
    fact(
        "now ready",
        inbox
            .ready_tasks()
            .first()
            .map(|t| t.title.clone())
            .unwrap_or_default(),
    );
    assert_eq!(inbox.ready_tasks()[0].id, notify_id);

    heading("or a person says no");
    let mut refused = TaskRegistry::from(vec![Task::new("delete the production database")]);
    let dangerous_id = refused.pending()[0].id;
    let partial: Trace<String> = Trace::failed(TraceErr::other("needs sign-off"));
    refused.pause(
        dangerous_id,
        ApprovalRequest::new("really?", partial.trace_ref()),
        &store,
    )?;
    refused.resume(
        dangerous_id,
        ApprovalDecision::reject("joe", "absolutely not"),
        &store,
    )?;
    let outcome = refused.get(dangerous_id).expect("the task survives");
    fact("status", format!("{:?}", outcome.status));

    std::fs::remove_file(&path).ok();
    Ok(())
}
