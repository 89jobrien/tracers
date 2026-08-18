use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use trace_lang_agent::Agent;

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
/// use trace_lang_runtime::AgentRegistry;
/// use trace_lang_agent::{Agent, AgentContext};
/// use trace_lang_core::Trace;
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
    /// Construct an empty registry.
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

    /// True if an agent is registered under `name`.
    pub fn contains(&self, name: &str) -> bool {
        self.agents.contains_key(name)
    }

    /// Number of registered agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// True if no agents are registered.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

impl<I, O> std::fmt::Debug for AgentRegistry<I, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRegistry")
            .field("agents", &self.agents.keys().collect::<Vec<_>>())
            .finish()
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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use trace_lang_agent::AgentContext;
    use trace_lang_core::Trace;

    struct Echo(&'static str);

    #[async_trait]
    impl Agent for Echo {
        type Input = String;
        type Output = String;
        fn name(&self) -> &str {
            self.0
        }
        fn goal(&self) -> &str {
            "echo input"
        }
        async fn run(&self, input: String, ctx: &mut AgentContext) -> Trace<String> {
            ctx.record_step().unwrap();
            Trace::new(input)
        }
    }

    #[test]
    fn empty_registry_reports_len_zero_and_is_empty() {
        let registry: AgentRegistry<String, String> = AgentRegistry::new();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
        assert!(!registry.contains("Echo"));
    }

    #[test]
    fn register_makes_agent_findable_by_name() {
        let mut registry: AgentRegistry<String, String> = AgentRegistry::new();
        registry.register(Arc::new(Echo("Echo")));
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        assert!(registry.contains("Echo"));
    }

    #[test]
    fn registering_same_name_twice_overwrites() {
        let mut registry: AgentRegistry<String, String> = AgentRegistry::new();
        registry.register(Arc::new(Echo("Echo")));
        registry.register(Arc::new(Echo("Echo")));
        assert_eq!(registry.len(), 1);
    }
}
