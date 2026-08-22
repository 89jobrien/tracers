//! `trace-lang-runtime` — agent registry, delegation resolution, parallel
//! fan-out, and speculative branching for trace:: pipelines.
//!
//! `trace-lang-agent` defines *what* an agent should do when it needs to
//! escalate (`EscalationAction::Delegate("SeniorCoder")`), but
//! resolving a name into a live agent and actually running it is a
//! runtime concern — that's what this crate adds:
//!
//! - [`AgentRegistry`] — a name → agent lookup table
//! - [`run_with_escalation`] — runs an agent and automatically resolves
//!   any `Delegate` escalation against a registry, hopping up to a
//!   caller-supplied limit
//! - [`join_all`] — runs one agent concurrently over many inputs
//! - [`speculate()`] — runs several *different* candidate agents
//!   concurrently over the same input and picks a winner by confidence,
//!   recording the losers as rejected [`trace_lang_core::Branch`]es
//! - [`speculate_race`] — the same fan-out, but stops as soon as one
//!   candidate clears a confidence threshold and cancels the rest, so the
//!   accuracy/latency/cost trade is a parameter rather than a fork in the
//!   API
//!
//! # Known limitation
//!
//! [`join_all`] and [`speculate()`] use `futures::future::join_all` (and
//! [`speculate_race`] a `FuturesUnordered`), all of which poll
//! concurrently on the current task rather than distributing across OS
//! threads. True multi-threaded parallelism
//! (via `tokio::spawn` and `'static` agents) and a shared step-budget
//! that spans concurrent branches are both tracked as future work —
//! see this repo's `HANDOFF` for the open items.

pub mod execute;
pub mod join;
pub mod registry;
pub mod speculate;

/// Real agent fixtures for shared use across this crate's tests and, via
/// the `test-support` feature, downstream crates' tests (see
/// `crates/task/src/checkpoint/mod.rs`'s `conformance` module for the
/// identical precedent).
#[cfg(any(test, feature = "test-support"))]
pub mod fixtures;

pub use execute::{RunOutcome, run_with_escalation};
pub use join::join_all;
pub use registry::AgentRegistry;
pub use speculate::{speculate, speculate_race};
