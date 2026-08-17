use std::time::{Duration, Instant};

/// RAII-style timing span. Timing begins on construction and is captured
/// on `finish()` or `Drop`. Mirrors the `agent_trace` crate's span pattern.
///
/// ```rust
/// use trace_lang_core::Span;
///
/// let span = Span::start("search");
/// // ... do work ...
/// let duration = span.finish();
/// ```
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
