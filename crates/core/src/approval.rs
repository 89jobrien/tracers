use crate::trace::TraceRef;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A question a pipeline stopped to ask a human.
///
/// This is not a delegation to a "HumanReviewer" agent — that already works
/// today through `EscalationAction::Delegate` and is fine when "human" is
/// really just another named endpoint. An `ApprovalRequest` represents a
/// genuine stop-the-world pause: the pipeline serializes, nothing runs, and
/// the decision arrives minutes or days later through some external channel
/// (a CLI prompt, a Slack message, a web form).
///
/// The request carries the [`TraceRef`] of the partial trace that reached the
/// pause, so an approver is never asked to decide without the provenance that
/// led there — the same rule `TaskStatus::Done` follows.
///
/// ```rust
/// use trace_lang_core::{ApprovalRequest, Trace};
///
/// let partial = Trace::new("draft refund of $4,000");
/// let request = ApprovalRequest::new("approve this refund?", partial.trace_ref())
///     .with_context(serde_json::json!({ "amount_usd": 4000 }));
///
/// assert_eq!(request.trace, partial.trace_ref());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Stable id, so an out-of-band channel can correlate its answer back
    /// to the exact request that was asked.
    pub id: Uuid,
    /// What the human is being asked.
    pub question: String,
    /// Whatever the agent wants the human to see alongside the question.
    /// `Value::Null` when nothing was attached.
    pub context: serde_json::Value,
    /// The partial trace that reached the pause.
    pub trace: TraceRef,
    pub requested_at: DateTime<Utc>,
}

impl ApprovalRequest {
    /// Ask `question` about the run recorded in `trace`, timestamped now.
    pub fn new(question: impl Into<String>, trace: TraceRef) -> Self {
        Self {
            id: Uuid::new_v4(),
            question: question.into(),
            context: serde_json::Value::Null,
            trace,
            requested_at: Utc::now(),
        }
    }

    /// A request raised from inside a lifecycle hook, which has no access
    /// to the trace it is escalating from.
    ///
    /// The `trace` field is left unattached (a nil `TraceRef`);
    /// `spawn`/`delegate` in `trace-lang-agent` stamp the real one before
    /// handing the escalation back, so a caller never sees an unattached
    /// request. Construct with [`Self::new`] anywhere the trace is known.
    pub fn unattached(question: impl Into<String>) -> Self {
        Self::new(question, TraceRef(Uuid::nil()))
    }

    /// True if this request points at a real trace rather than the
    /// placeholder [`Self::unattached`] leaves behind.
    pub fn is_attached(&self) -> bool {
        !self.trace.0.is_nil()
    }

    /// Point this request at `trace`, but only if it is still unattached —
    /// a request that already names its trace is left alone.
    pub fn attach(&mut self, trace: TraceRef) {
        if !self.is_attached() {
            self.trace = trace;
        }
    }

    /// Builder: attach the context a human needs to answer.
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = context;
        self
    }

    /// How long this request has been waiting, as of now. A checkpoint that
    /// has sat unanswered for long enough may be resuming into a world that
    /// has moved on — the caller decides what "long enough" means.
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.requested_at
    }
}

/// A human's answer to an [`ApprovalRequest`].
///
/// Both variants record *who* decided: an approval nobody is accountable for
/// is not much of an approval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    /// Proceed. The paused work becomes schedulable again.
    Approved { by: String, note: Option<String> },
    /// Do not proceed. The paused work terminates as failed.
    Rejected { by: String, reason: String },
}

impl ApprovalDecision {
    /// Approve with no further comment.
    pub fn approve(by: impl Into<String>) -> Self {
        Self::Approved {
            by: by.into(),
            note: None,
        }
    }

    /// Approve, recording a note alongside the decision.
    pub fn approve_with_note(by: impl Into<String>, note: impl Into<String>) -> Self {
        Self::Approved {
            by: by.into(),
            note: Some(note.into()),
        }
    }

    /// Reject, recording why.
    pub fn reject(by: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Rejected {
            by: by.into(),
            reason: reason.into(),
        }
    }

    /// True if the decision was to proceed.
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved { .. })
    }

    /// Who made the decision.
    pub fn decided_by(&self) -> &str {
        match self {
            Self::Approved { by, .. } | Self::Rejected { by, .. } => by,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_request() -> ApprovalRequest {
        ApprovalRequest::new("ship it?", TraceRef(Uuid::new_v4()))
    }

    #[test]
    fn a_new_request_has_no_context_until_one_is_attached() {
        let request = a_request();
        assert_eq!(request.context, serde_json::Value::Null);

        let with_context = request.with_context(serde_json::json!({ "diff": "+1 -1" }));
        assert_eq!(with_context.context["diff"], "+1 -1");
    }

    #[test]
    fn a_request_carries_the_trace_that_reached_the_pause() {
        let trace = TraceRef(Uuid::new_v4());
        assert_eq!(ApprovalRequest::new("?", trace.clone()).trace, trace);
    }

    #[test]
    fn an_unattached_request_is_stamped_once_and_then_left_alone() {
        let mut request = ApprovalRequest::unattached("ship it?");
        assert!(!request.is_attached());

        let real = TraceRef(Uuid::new_v4());
        request.attach(real.clone());
        assert!(request.is_attached());
        assert_eq!(request.trace, real);

        // A second stamp must not rewrite history.
        request.attach(TraceRef(Uuid::new_v4()));
        assert_eq!(request.trace, real);
    }

    #[test]
    fn a_request_built_with_a_known_trace_is_already_attached() {
        let request = ApprovalRequest::new("?", TraceRef(Uuid::new_v4()));
        assert!(request.is_attached());
    }

    #[test]
    fn age_is_non_negative_for_a_request_created_now() {
        assert!(a_request().age() >= chrono::Duration::zero());
    }

    #[test]
    fn approve_and_reject_record_who_decided() {
        let approved = ApprovalDecision::approve("joe");
        assert!(approved.is_approved());
        assert_eq!(approved.decided_by(), "joe");

        let rejected = ApprovalDecision::reject("joe", "budget not signed off");
        assert!(!rejected.is_approved());
        assert_eq!(rejected.decided_by(), "joe");
    }

    #[test]
    fn approve_with_note_keeps_the_note() {
        let decision = ApprovalDecision::approve_with_note("joe", "checked with finance");
        assert_eq!(
            decision,
            ApprovalDecision::Approved {
                by: "joe".to_string(),
                note: Some("checked with finance".to_string()),
            }
        );
    }

    #[test]
    fn a_request_round_trips_through_json() {
        // The whole point of a pause is that it survives serialization —
        // the decision may arrive days after the process that asked exits.
        let request = a_request().with_context(serde_json::json!({ "amount": 42 }));
        let json = serde_json::to_string(&request).expect("serializes");
        let restored: ApprovalRequest = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(restored, request);
    }
}
