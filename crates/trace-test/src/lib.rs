//! `trace-test` — assert the *shape* of an agent execution, not just its
//! final output. `assert_trace!` inspects `Trace::causal_chain()` and
//! `AgentContext::delegation_chain` via the `TraceOutcome` port, so it
//! works uniformly over `SpawnOutcome<O>` and `RunOutcome<O>`.

pub mod assertion;
pub mod outcome;

pub use assertion::{
    TraceAssertionError, confidence_below, contains_step, escalates_to, never_step,
};
pub use outcome::TraceOutcome;
