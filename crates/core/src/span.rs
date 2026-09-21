//! Measures elapsed time for named execution regions.

use std::time::{Duration, Instant};

/// Timing span that starts on construction and reports elapsed time through
/// [`Span::elapsed`] or [`Span::finish`].
///
/// ```rust
/// use trace_lang_core::Span;
///
/// let span = Span::start("search");
/// // ... do work ...
/// let duration = span.finish();
/// ```
#[derive(Debug, Clone)]
pub struct Span {
    /// Label identifying the timed region.
    pub name: String,
    started: Instant,
}

impl Span {
    /// Start timing a span named `name`, beginning immediately.
    pub fn start(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            started: Instant::now(),
        }
    }

    /// Consume the span and return the elapsed duration.
    pub fn finish(self) -> Duration {
        self.started.elapsed()
    }

    /// Peek at elapsed time without consuming the span.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}
