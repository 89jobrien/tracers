use tracers_core::TraceErr;

/// The declarative outcome of a lifecycle hook
/// (`on_low_confidence`, `on_budget_exceeded`, `on_step_failure`).
///
/// Agents return an `EscalationAction` describing *what should happen*;
/// the caller (typically `spawn`, or an orchestrating agent) is
/// responsible for actually carrying it out — e.g. calling `delegate()`
/// with the named agent.
#[derive(Debug, Clone, PartialEq)]
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
    pub fn is_none(&self) -> bool {
        matches!(self, EscalationAction::None)
    }

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
}
