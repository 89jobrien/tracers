use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign};

/// What a single [`crate::Step`] cost to produce.
///
/// Token counts are always recorded; `dollars` is optional because the
/// price of those tokens depends on a provider pricing table that is not
/// part of `trace-lang-core`. Recording the dollar figure at step time
/// (rather than computing it lazily at query time) means a checkpoint
/// written today still reports what the run actually cost even after the
/// provider changes its prices — historical spend is a fact about the
/// past, not a function of today's price list.
///
/// ```rust
/// use trace_lang_core::{Step, StepCost, Trace};
///
/// let mut t = Trace::new("summary");
/// t.push_step(
///     Step::named("summarize")
///         .with_cost(StepCost::new(1_200, 340).with_dollars(0.0042)),
/// );
///
/// assert_eq!(t.total_cost().total_tokens(), 1_540);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct StepCost {
    /// Tokens fed into the step (prompt, context, tool output).
    pub input_tokens: u64,
    /// Tokens produced by the step (completion).
    pub output_tokens: u64,
    /// Dollar cost, if the caller knew the provider's price at call time.
    /// `None` means "not recorded", never "free".
    pub dollars: Option<f64>,
}

impl StepCost {
    /// Record a token cost with no dollar figure attached.
    pub fn new(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            dollars: None,
        }
    }

    /// Builder: attach a dollar cost. Negative values (and `NaN`) clamp to
    /// `0.0` — the same clamping convention `with_confidence` uses.
    pub fn with_dollars(mut self, dollars: f64) -> Self {
        self.dollars = Some(dollars.max(0.0));
        self
    }

    /// Input plus output tokens — the metric `priciest_steps` falls back to
    /// when no dollar figures were recorded.
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }

    /// True if nothing was actually recorded — no tokens and no dollars.
    pub fn is_zero(&self) -> bool {
        self.input_tokens == 0 && self.output_tokens == 0 && self.dollars.is_none()
    }
}

/// Summing costs saturates token counts and keeps `dollars` as `Some` iff
/// at least one operand recorded one — summing a priced step with an
/// unpriced one reports the price that *is* known rather than discarding it.
impl Add for StepCost {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let dollars = match (self.dollars, rhs.dollars) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
        };
        Self {
            input_tokens: self.input_tokens.saturating_add(rhs.input_tokens),
            output_tokens: self.output_tokens.saturating_add(rhs.output_tokens),
            dollars,
        }
    }
}

impl AddAssign for StepCost {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::iter::Sum for StepCost {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |acc, c| acc + c)
    }
}

impl std::fmt::Display for StepCost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} in / {} out", self.input_tokens, self.output_tokens)?;
        if let Some(dollars) = self.dollars {
            write!(f, " (${dollars:.4})")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_records_tokens_and_leaves_dollars_unrecorded() {
        let cost = StepCost::new(10, 5);
        assert_eq!(cost.total_tokens(), 15);
        assert_eq!(cost.dollars, None);
    }

    #[test]
    fn with_dollars_clamps_negative_and_nan_to_zero() {
        assert_eq!(StepCost::new(0, 0).with_dollars(-1.0).dollars, Some(0.0));
        assert_eq!(
            StepCost::new(0, 0).with_dollars(f64::NAN).dollars,
            Some(0.0)
        );
    }

    #[test]
    fn add_sums_tokens_and_keeps_a_known_price_over_an_unknown_one() {
        let priced = StepCost::new(10, 5).with_dollars(0.25);
        let unpriced = StepCost::new(1, 1);

        let total = priced + unpriced;
        assert_eq!(total.input_tokens, 11);
        assert_eq!(total.output_tokens, 6);
        assert_eq!(total.dollars, Some(0.25));
    }

    #[test]
    fn add_leaves_dollars_unrecorded_when_neither_side_recorded_one() {
        let total = StepCost::new(1, 1) + StepCost::new(2, 2);
        assert_eq!(total.dollars, None);
    }

    #[test]
    fn token_addition_saturates_instead_of_overflowing() {
        let huge = StepCost::new(u64::MAX, u64::MAX);
        let total = huge + StepCost::new(1, 1);
        assert_eq!(total.input_tokens, u64::MAX);
        assert_eq!(huge.total_tokens(), u64::MAX);
    }

    #[test]
    fn sum_folds_an_iterator_of_costs() {
        let total: StepCost = [
            StepCost::new(1, 1).with_dollars(0.1),
            StepCost::new(2, 2).with_dollars(0.2),
        ]
        .into_iter()
        .sum();

        assert_eq!(total.total_tokens(), 6);
        assert_eq!(total.dollars.map(|d| (d * 100.0).round()), Some(30.0));
    }

    #[test]
    fn is_zero_only_when_nothing_was_recorded() {
        assert!(StepCost::default().is_zero());
        assert!(!StepCost::new(0, 0).with_dollars(0.0).is_zero());
        assert!(!StepCost::new(1, 0).is_zero());
    }

    #[test]
    fn display_shows_tokens_and_an_optional_price() {
        assert_eq!(StepCost::new(10, 5).to_string(), "10 in / 5 out");
        assert_eq!(
            StepCost::new(10, 5).with_dollars(0.25).to_string(),
            "10 in / 5 out ($0.2500)"
        );
    }
}
