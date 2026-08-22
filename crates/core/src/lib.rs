//! `trace-lang-core` — reasoning provenance as a first-class value.
//!
//! Every computation in a trace:: program returns `Trace<T>` rather than bare `T`.
//! The trace carries the full causal chain: steps taken, branches rejected,
//! confidence at each decision point, and RAII-style span timing.
//!
//! # Quick start
//!
//! ```rust
//! use trace_lang_core::{Trace, Step, TraceErr};
//!
//! let mut t = Trace::new("hello world");
//! t.push_step(Step::named("greet").with_confidence(0.97));
//!
//! assert_eq!(t.value(), Some(&"hello world"));
//! assert_eq!(t.causal_chain().len(), 1);
//! ```

// TODO: add an `examples/` directory demonstrating Trace<T>/Task/Agent usage
// end-to-end — the doctest above is the only runnable usage sample in the
// whole workspace despite all four crates being "usable as a library today".
//
// TODO: add benches/ (criterion) and fuzz/ (cargo-fuzz) — `taskit bench` and
// `taskit fuzz` are both available subcommands already, currently unused.
// A good fuzz target: Trace<T>/Task (de)serialization round-tripping, given
// CLAUDE.md's "all types are Serialize + Deserialize — a compile-time
// constraint" invariant.

pub mod cost;
pub mod error;
pub mod graph;
pub mod span;
pub mod step;
pub mod trace;

pub use cost::StepCost;
pub use error::TraceErr;
pub use graph::{TraceGraph, TraceNode};
pub use span::Span;
pub use step::{Branch, BranchOutcome, Step, StepOutcome};
pub use trace::{Trace, TraceRef};
