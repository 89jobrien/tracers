use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

/// Every error variant in trace:: is named and recorded in the trace.
/// There are no silent panics — a failing step always produces a `TraceErr`
/// that propagates via `?` exactly like `Result<T, E>`, but is logged at
/// every propagation point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum TraceErr {
    /// A step called `reject()` — the branch was explicitly abandoned.
    #[error("rejected: {0}")]
    Rejected(String),

    /// An external tool call returned an error.
    #[error("tool failed: {tool} — {message}")]
    ToolFailed { tool: String, message: String },

    /// The agent exceeded its declared step budget.
    #[error("budget exhausted after {steps} steps")]
    BudgetExhausted { steps: usize },

    /// A delegated agent returned an error. The inner UUID is that agent's trace id.
    #[error("delegation failed (trace {trace_id}): {message}")]
    DelegationFailed { trace_id: Uuid, message: String },

    /// A step's confidence score fell below the agent's declared threshold.
    #[error("confidence too low: {score:.2} (threshold {threshold:.2})")]
    LowConfidence { score: f64, threshold: f64 },

    /// A step exceeded its time limit.
    #[error("step timed out after {duration:?}")]
    Timeout { duration: Duration },

    /// Serialization/deserialization failure (task or trace checkpoint).
    #[error("serialization error: {0}")]
    Serde(String),

    /// Catch-all for errors that don't fit the above.
    #[error("{0}")]
    Other(String),
}

impl TraceErr {
    pub fn rejected(reason: impl Into<String>) -> Self {
        Self::Rejected(reason.into())
    }

    pub fn tool_failed(tool: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ToolFailed {
            tool: tool.into(),
            message: message.into(),
        }
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}
