use serde::{Deserialize, Serialize};
use trace_lang_core::{ApprovalRequest, TraceErr};

/// The declarative outcome of a lifecycle hook
/// (`on_low_confidence`, `on_budget_exceeded`, `on_step_failure`).
///
/// Agents return an `EscalationAction` describing *what should happen*;
/// the caller (typically `spawn`, or an orchestrating agent) is
/// responsible for actually carrying it out — e.g. calling `delegate()`
/// with the named agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EscalationAction {
    /// No escalation — proceed as normal.
    None,
    /// Hand off to another agent, identified by name. The caller
    /// resolves the name to an actual `Agent` instance.
    Delegate(String),
    /// Abort with the given error rather than escalating further.
    Emit(TraceErr),
    /// Stop and wait for a human. Unlike `Delegate`, no agent can
    /// discharge this — the caller must park the work (typically as
    /// `TaskStatus::Paused` in a checkpointed `TaskRegistry`) and resume
    /// it once a decision arrives through an external channel.
    RequireApproval(ApprovalRequest),
}

impl EscalationAction {
    /// True if this action is `None` — no escalation recommended.
    pub fn is_none(&self) -> bool {
        matches!(self, EscalationAction::None)
    }

    /// The delegation target's name, if this action is `Delegate`.
    pub fn delegate_target(&self) -> Option<&str> {
        match self {
            EscalationAction::Delegate(name) => Some(name),
            _ => None,
        }
    }

    /// The question awaiting a human, if this action is `RequireApproval`.
    pub fn approval_request(&self) -> Option<&ApprovalRequest> {
        match self {
            EscalationAction::RequireApproval(request) => Some(request),
            _ => None,
        }
    }

    /// True if no agent in any registry can discharge this action — it
    /// needs a human, or it is a terminal error. `run_with_escalation`
    /// uses this to decide what to hand back as `unresolved`.
    pub fn needs_a_human(&self) -> bool {
        matches!(
            self,
            EscalationAction::RequireApproval(_) | EscalationAction::Emit(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_variant_is_none_and_has_no_target() {
        let action = EscalationAction::None;
        assert!(action.is_none());
        assert_eq!(action.delegate_target(), None);
    }

    #[test]
    fn delegate_variant_is_not_none_and_exposes_target() {
        let action = EscalationAction::Delegate("Senior".to_string());
        assert!(!action.is_none());
        assert_eq!(action.delegate_target(), Some("Senior"));
    }

    #[test]
    fn emit_variant_is_not_none_and_has_no_target() {
        let action = EscalationAction::Emit(TraceErr::other("abort"));
        assert!(!action.is_none());
        assert_eq!(action.delegate_target(), None);
        assert!(action.needs_a_human());
    }

    #[test]
    fn require_approval_exposes_its_request_and_no_delegate_target() {
        let partial = trace_lang_core::Trace::new("draft");
        let request = ApprovalRequest::new("ship it?", partial.trace_ref());
        let action = EscalationAction::RequireApproval(request.clone());

        assert!(!action.is_none());
        assert_eq!(action.delegate_target(), None);
        assert_eq!(action.approval_request(), Some(&request));
        assert!(action.needs_a_human());
    }

    #[test]
    fn delegation_and_no_escalation_do_not_need_a_human() {
        assert!(!EscalationAction::None.needs_a_human());
        assert!(!EscalationAction::Delegate("Senior".to_string()).needs_a_human());
    }
}
