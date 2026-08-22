use serde::{Deserialize, Serialize};
use trace_lang_core::TraceErr;

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
    }

    #[test]
    fn escalation_action_serde_round_trips_every_variant() {
        for action in [
            EscalationAction::None,
            EscalationAction::Delegate("Senior".to_string()),
            EscalationAction::Emit(TraceErr::other("abort")),
        ] {
            let json = serde_json::to_string(&action).unwrap();
            let back: EscalationAction = serde_json::from_str(&json).unwrap();
            assert_eq!(back, action);
        }
    }
}
