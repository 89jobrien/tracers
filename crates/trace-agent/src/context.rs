use trace_core::TraceErr;

/// Per-run state threaded through [`crate::Agent::run`].
///
/// Tracks step count against the agent's declared budget and carries
/// the delegation chain — the ordered list of agent names that handed
/// off execution to reach this point. `spawn()` starts a fresh chain;
/// `delegate()` extends the caller's chain.
#[derive(Debug, Clone)]
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
        if let Some(budget) = self.budget {
            if self.steps_taken > budget {
                return Err(TraceErr::BudgetExhausted {
                    steps: self.steps_taken,
                });
            }
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
