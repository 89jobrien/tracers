//! `tracers-core` — reasoning provenance as a first-class value.
//!
//! Every computation in a trace:: program returns `Trace<T>` rather than bare `T`.
//! The trace carries the full causal chain: steps taken, branches rejected,
//! confidence at each decision point, and RAII-style span timing.
//!
//! # Quick start
//!
//! ```rust
//! use tracers_core::{Trace, Step, TraceErr};
//!
//! let mut t = Trace::new("hello world");
//! t.push_step(Step::named("greet").with_confidence(0.97));
//!
//! assert_eq!(t.value(), Some(&"hello world"));
//! assert_eq!(t.causal_chain().len(), 1);
//! ```

pub mod error;
pub mod span;
pub mod step;
pub mod trace;

pub use error::TraceErr;
pub use span::Span;
pub use step::{Branch, BranchOutcome, Step, StepOutcome};
pub use trace::{Trace, TraceRef};
