use serde::{Deserialize, Serialize};
use trace_lang_core::TraceErr;

/// Per-run state threaded through [`crate::Agent::run`].
///
/// Tracks step count against the agent's declared budget and carries
/// the delegation chain — the ordered list of agent names that handed
/// off execution to reach this point. `spawn()` starts a fresh chain;
/// `delegate()` extends the caller's chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub agent_name: String,
    pub steps_taken: usize,
    pub budget: Option<usize>,
    pub delegation_chain: Vec<String>,
}

impl AgentContext {
    /// Start a fresh context for `agent_name` with the given budget.
    pub fn new(agent_name: impl Into<String>, budget: Option<usize>) -> Self {
        let name = agent_name.into();
        Self {
            delegation_chain: vec![name.clone()],
            agent_name: name,
            steps_taken: 0,
            budget,
        }
    }

    /// Record that a step was taken. Returns
    /// [`TraceErr::BudgetExhausted`] once `steps_taken` would exceed
    /// the declared budget.
    pub fn record_step(&mut self) -> Result<(), TraceErr> {
        self.steps_taken += 1;
        if let Some(budget) = self.budget
            && self.steps_taken > budget
        {
            return Err(TraceErr::BudgetExhausted {
                steps: self.steps_taken,
            });
        }
        Ok(())
    }

    /// Steps remaining before the budget is exhausted. `None` if the
    /// agent has no budget.
    pub fn budget_remaining(&self) -> Option<usize> {
        self.budget.map(|b| b.saturating_sub(self.steps_taken))
    }

    /// `true` once `steps_taken` has reached the declared budget.
    pub fn is_budget_exhausted(&self) -> bool {
        matches!(self.budget, Some(b) if self.steps_taken >= b)
    }

    /// Extend this context's delegation chain with a new agent name,
    /// for use when delegating to a sub-agent.
    pub(crate) fn extend_chain(&self, next_agent: &str) -> Vec<String> {
        let mut chain = self.delegation_chain.clone();
        chain.push(next_agent.to_string());
        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_context_starts_at_zero_steps_with_a_singleton_chain() {
        let ctx = AgentContext::new("probe", Some(5));
        assert_eq!(ctx.steps_taken, 0);
        assert_eq!(ctx.delegation_chain, vec!["probe"]);
        assert!(!ctx.is_budget_exhausted());
    }

    #[test]
    fn record_step_without_a_budget_never_errors() {
        let mut ctx = AgentContext::new("probe", None);
        for _ in 0..1000 {
            ctx.record_step().expect("no budget means no limit");
        }
        assert_eq!(ctx.budget_remaining(), None);
        assert!(!ctx.is_budget_exhausted());
    }

    #[test]
    fn record_step_errors_exactly_one_step_past_budget() {
        let mut ctx = AgentContext::new("probe", Some(2));
        ctx.record_step().expect("first step is within budget");
        assert!(!ctx.is_budget_exhausted());
        ctx.record_step()
            .expect("second step reaches budget exactly");
        assert!(ctx.is_budget_exhausted());
        let err = ctx.record_step().expect_err("third step exceeds budget");
        assert!(matches!(
            err,
            trace_lang_core::TraceErr::BudgetExhausted { steps: 3 }
        ));
    }

    #[test]
    fn budget_remaining_saturates_at_zero_rather_than_going_negative() {
        let mut ctx = AgentContext::new("probe", Some(1));
        ctx.record_step().unwrap();
        assert_eq!(ctx.budget_remaining(), Some(0));
        let _ = ctx.record_step();
        assert_eq!(ctx.budget_remaining(), Some(0));
    }

    #[test]
    fn extend_chain_appends_without_mutating_the_original() {
        let ctx = AgentContext::new("root", None);
        let extended = ctx.extend_chain("child");
        assert_eq!(extended, vec!["root", "child"]);
        assert_eq!(ctx.delegation_chain, vec!["root"]);
    }

    proptest::proptest! {
        #[test]
        fn budget_remaining_never_exceeds_the_declared_budget(
            budget in 0usize..50,
            calls in 0usize..80,
        ) {
            let mut ctx = AgentContext::new("probe", Some(budget));
            for _ in 0..calls {
                let _ = ctx.record_step();
            }
            let remaining = ctx.budget_remaining().unwrap();
            proptest::prop_assert!(remaining <= budget);
        }

        #[test]
        fn is_budget_exhausted_matches_steps_taken_reaching_budget(
            budget in 0usize..50,
            calls in 0usize..80,
        ) {
            let mut ctx = AgentContext::new("probe", Some(budget));
            for _ in 0..calls {
                let _ = ctx.record_step();
            }
            proptest::prop_assert_eq!(ctx.is_budget_exhausted(), ctx.steps_taken >= budget);
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Proves `budget_remaining`'s `saturating_sub` never underflows and
    /// always agrees with `is_budget_exhausted` — the same
    /// steps_taken-vs-budget comparison done two different ways must
    /// never disagree, for any reachable budget/steps_taken pair.
    #[kani::proof]
    fn budget_remaining_and_is_exhausted_agree() {
        let budget: usize = kani::any();
        let steps_taken: usize = kani::any();
        kani::assume(budget <= 1_000_000);
        kani::assume(steps_taken <= 1_000_000);

        let ctx = AgentContext {
            agent_name: String::new(),
            steps_taken,
            budget: Some(budget),
            delegation_chain: Vec::new(),
        };

        let remaining = ctx.budget_remaining().unwrap();
        let exhausted = ctx.is_budget_exhausted();

        assert_eq!(remaining == 0, steps_taken >= budget);
        assert_eq!(exhausted, steps_taken >= budget);
    }
}
