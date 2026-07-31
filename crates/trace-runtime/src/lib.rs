//! `trace-runtime` — agent registry, delegation resolution, parallel
//! fan-out, and speculative branching for trace:: pipelines.
//!
//! `trace-agent` defines *what* an agent should do when it needs to
//! escalate (`EscalationAction::Delegate("SeniorCoder")`), but
//! resolving a name into a live agent and actually running it is a
//! runtime concern — that's what this crate adds:
//!
//! - [`AgentRegistry`] — a name → agent lookup table
//! - [`run_with_escalation`] — runs an agent and automatically resolves
//!   any `Delegate` escalation against a registry, hopping up to a
//!   caller-supplied limit
//! - [`join_all`] — runs one agent concurrently over many inputs
//! - [`speculate`] — runs several *different* candidate agents
//!   concurrently over the same input and picks a winner by confidence,
//!   recording the losers as rejected [`trace_core::Branch`]es
//!
//! # Known limitation
//!
//! [`join_all`] and [`speculate`] use `futures::future::join_all`,
//! which polls concurrently on the current task rather than
//! distributing across OS threads. True multi-threaded parallelism
//! (via `tokio::spawn` and `'static` agents) and a shared step-budget
//! that spans concurrent branches are both tracked as future work —
//! see this repo's `HANDOFF` for the open items.

pub mod execute;
pub mod join;
pub mod registry;
pub mod speculate;

pub use execute::{run_with_escalation, RunOutcome};
pub use join::join_all;
pub use registry::AgentRegistry;
pub use speculate::speculate;
