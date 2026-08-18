use miette::Diagnostic;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

/// Every error variant in trace:: is named and recorded in the trace.
/// There are no silent panics — a failing step always produces a `TraceErr`
/// that propagates via `?` exactly like `Result<T, E>`, but is logged at
/// every propagation point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error, Diagnostic)]
pub enum TraceErr {
    /// A step called `reject()` — the branch was explicitly abandoned.
    #[error("rejected: {0}")]
    #[diagnostic(
        code(trace::rejected),
        help(
            "this branch was explicitly abandoned via reject() — inspect the reason and, if unintended, adjust the step logic that called it"
        )
    )]
    Rejected(String),

    /// An external tool call returned an error.
    #[error("tool failed: {tool} — {message}")]
    #[diagnostic(
        code(trace::tool_failed),
        help(
            "check the tool's availability and input, then retry the step or fall back to another tool"
        )
    )]
    ToolFailed { tool: String, message: String },

    /// The agent exceeded its declared step budget.
    #[error("budget exhausted after {steps} steps")]
    #[diagnostic(
        code(trace::budget_exhausted),
        help("increase the agent's declared step budget")
    )]
    BudgetExhausted { steps: usize },

    /// A delegated agent returned an error. The inner UUID is that agent's trace id.
    #[error("delegation failed (trace {trace_id}): {message}")]
    #[diagnostic(
        code(trace::delegation_failed),
        help(
            "inspect the delegate's trace ({trace_id}) for the root cause — the failure originated downstream, not in the delegating agent"
        )
    )]
    DelegationFailed { trace_id: Uuid, message: String },

    /// A step's confidence score fell below the agent's declared threshold.
    #[error("confidence too low: {score:.2} (threshold {threshold:.2})")]
    #[diagnostic(
        code(trace::low_confidence),
        help(
            "either lower the agent's confidence threshold or improve the step's inputs so it can produce a higher-confidence result"
        )
    )]
    LowConfidence { score: f64, threshold: f64 },

    /// A step exceeded its time limit.
    #[error("step timed out after {duration:?}")]
    #[diagnostic(
        code(trace::timeout),
        help(
            "raise the step's time limit if the work is legitimately slow, or investigate why it hung"
        )
    )]
    Timeout { duration: Duration },

    /// Serialization/deserialization failure (task or trace checkpoint).
    #[error("serialization error: {0}")]
    #[diagnostic(
        code(trace::serde),
        help(
            "the underlying serde_json error is embedded in the message above — check for a schema mismatch between the checkpoint on disk and the current type definitions"
        )
    )]
    Serde(String),

    /// Catch-all for errors that don't fit the above.
    #[error("{0}")]
    #[diagnostic(
        code(trace::other),
        help(
            "this is an uncategorized TraceErr — consider adding a dedicated variant if this failure mode recurs"
        )
    )]
    Other(String),
}

impl TraceErr {
    /// Build a `Rejected` error from a reason string.
    pub fn rejected(reason: impl Into<String>) -> Self {
        Self::Rejected(reason.into())
    }

    /// Build a `ToolFailed` error from a tool name and failure message.
    pub fn tool_failed(tool: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ToolFailed {
            tool: tool.into(),
            message: message.into(),
        }
    }

    /// Build a catch-all `Other` error for cases that don't fit a specific variant.
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_err_is_a_miette_diagnostic() {
        fn assert_diagnostic<T: miette::Diagnostic>() {}
        assert_diagnostic::<TraceErr>();
    }
}
