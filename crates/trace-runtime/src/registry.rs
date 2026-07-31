use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use trace_agent::Agent;

/// A runtime lookup table from agent name to a live [`Agent`] instance.
///
/// `EscalationAction::Delegate(name)` is just a string — resolving it
/// into an actual agent to hand off to is the registry's job. All
/// agents registered under a given `AgentRegistry<I, O>` must share the
/// same `Input`/`Output` contract, since a delegation target (a
/// reviewer, a fallback, a specialist) needs to accept the same task
/// shape as the agent that escalated to it.
///
/// ```rust
/// use trace_runtime::AgentRegistry;
/// use trace_agent::{Agent, AgentContext};
/// use trace_core::Trace;
/// use async_trait::async_trait;
/// use std::sync::Arc;
///
/// struct Fallback;
///
/// #[async_trait]
/// impl Agent for Fallback {
///     type Input = String;
///     type Output = String;
///     fn name(&self) -> &str { "Fallback" }
///     fn goal(&self) -> &str { "handle what the primary agent could not" }
///     async fn run(&self, input: String, ctx: &mut AgentContext) -> Trace<String> {
///         ctx.record_step().unwrap();
///         Trace::new(format!("fallback handled: {input}"))
///     }
/// }
///
/// let mut registry: AgentRegistry<String, String> = AgentRegistry::new();
/// registry.register(Arc::new(Fallback));
/// assert!(registry.get("Fallback").is_some());
/// assert!(registry.get("Unknown").is_none());
/// ```
pub struct AgentRegistry<I, O> {
    agents: HashMap<String, Arc<dyn Agent<Input = I, Output = O>>>,
}

impl<I, O> AgentRegistry<I, O>
where
    I: Send,
    O: Clone + Serialize + Send,
{
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Register an agent under its own [`Agent::name`]. Overwrites any
    /// existing registration with the same name.
    pub fn register(&mut self, agent: Arc<dyn Agent<Input = I, Output = O>>) {
        self.agents.insert(agent.name().to_string(), agent);
    }

    /// Look up a registered agent by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Agent<Input = I, Output = O>>> {
        self.agents.get(name).cloned()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.agents.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.agents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

impl<I, O> Default for AgentRegistry<I, O>
where
    I: Send,
    O: Clone + Serialize + Send,
{
    fn default() -> Self {
        Self::new()
    }
}
