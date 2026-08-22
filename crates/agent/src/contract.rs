use trace_lang_core::{Step, TraceErr};

type Check<T> = Box<dyn Fn(&T) -> Result<(), String> + Send + Sync>;

/// Design-by-contract for a single agent step.
///
/// A step can be *technically* successful — no `TraceErr`, no panic — and
/// still substantively wrong: an empty summary, a malformed identifier, a
/// number outside its valid range. `Contract` turns that class of failure
/// into a named, recorded trace event instead of a bug someone files after
/// noticing bad output three hops downstream.
///
/// A violation produces [`TraceErr::ContractViolated`] rather than a generic
/// failure, so [`crate::Agent::on_step_failure`] can distinguish *the tool
/// broke* from *the tool worked and returned something forbidden* — the two
/// deserve different escalations (retry vs. ask a human who understands the
/// invariant).
///
/// Checking is explicit at the call site: a contract is invoked from inside
/// `Agent::run`, so an agent in a hot loop simply doesn't call it, and there
/// is no global on/off switch to keep in sync.
///
/// ```rust
/// use trace_lang_agent::{Contract, contract_step};
/// use trace_lang_core::{Trace, TraceErr};
///
/// let contract: Contract<String, String> = Contract::new().post(|summary: &String| {
///     if summary.is_empty() {
///         Err("summary must not be empty".to_string())
///     } else {
///         Ok(())
///     }
/// });
///
/// let mut trace = Trace::new(String::new());
/// let checked = contract.check_post(&String::new());
/// trace.push_step(contract_step("summary-contract", &checked));
///
/// assert!(trace.causal_chain()[0].is_failed());
/// assert_eq!(
///     checked.unwrap_err().to_string(),
///     "contract violated: postcondition: summary must not be empty"
/// );
/// ```
pub struct Contract<I, O> {
    pre: Option<Check<I>>,
    post: Option<Check<O>>,
}

impl<I, O> Contract<I, O> {
    /// A contract that checks nothing. Add conditions with
    /// [`Self::pre`] and [`Self::post`].
    pub fn new() -> Self {
        Self {
            pre: None,
            post: None,
        }
    }

    /// Builder: declare what must be true of the step's input. Replaces any
    /// previously declared precondition.
    pub fn pre<F>(mut self, check: F) -> Self
    where
        F: Fn(&I) -> Result<(), String> + Send + Sync + 'static,
    {
        self.pre = Some(Box::new(check));
        self
    }

    /// Builder: declare what must be true of the step's output. Replaces any
    /// previously declared postcondition.
    pub fn post<F>(mut self, check: F) -> Self
    where
        F: Fn(&O) -> Result<(), String> + Send + Sync + 'static,
    {
        self.post = Some(Box::new(check));
        self
    }

    /// True if a precondition was declared.
    pub fn has_pre(&self) -> bool {
        self.pre.is_some()
    }

    /// True if a postcondition was declared.
    pub fn has_post(&self) -> bool {
        self.post.is_some()
    }

    /// Check `input` against the precondition. `Ok(())` if none was declared.
    pub fn check_pre(&self, input: &I) -> Result<(), TraceErr> {
        match &self.pre {
            Some(check) => check(input).map_err(|why| violated("precondition", why)),
            None => Ok(()),
        }
    }

    /// Check `output` against the postcondition. `Ok(())` if none was declared.
    pub fn check_post(&self, output: &O) -> Result<(), TraceErr> {
        match &self.post {
            Some(check) => check(output).map_err(|why| violated("postcondition", why)),
            None => Ok(()),
        }
    }
}

impl<I, O> Default for Contract<I, O> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, O> std::fmt::Debug for Contract<I, O> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Contract")
            .field("pre", &self.pre.is_some())
            .field("post", &self.post.is_some())
            .finish()
    }
}

fn violated(kind: &str, why: String) -> TraceErr {
    TraceErr::contract_violated(format!("{kind}: {why}"))
}

/// Turn a contract check into the [`Step`] that records it.
///
/// A satisfied contract is still worth a step — it is evidence the invariant
/// held at that point in the chain, which is what makes `causal_chain()`
/// readable as an audit trail rather than a list of things that went wrong.
///
/// ```rust
/// use trace_lang_agent::{Contract, contract_step};
///
/// let contract: Contract<(), u32> = Contract::new()
///     .post(|n: &u32| if *n > 0 { Ok(()) } else { Err("must be positive".into()) });
///
/// assert!(!contract_step("positive", &contract.check_post(&7)).is_failed());
/// assert!(contract_step("positive", &contract.check_post(&0)).is_failed());
/// ```
pub fn contract_step(name: impl Into<String>, outcome: &Result<(), TraceErr>) -> Step {
    let step = Step::named(name);
    match outcome {
        Ok(()) => step,
        Err(err) => step.failed(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn non_empty() -> Contract<String, String> {
        Contract::new()
            .pre(|input: &String| {
                if input.trim().is_empty() {
                    Err("input must not be blank".to_string())
                } else {
                    Ok(())
                }
            })
            .post(|output: &String| {
                if output.len() > 10 {
                    Err("output must be at most 10 chars".to_string())
                } else {
                    Ok(())
                }
            })
    }

    #[test]
    fn an_empty_contract_accepts_everything() {
        let contract: Contract<String, String> = Contract::new();
        assert!(!contract.has_pre());
        assert!(!contract.has_post());
        assert!(contract.check_pre(&String::new()).is_ok());
        assert!(contract.check_post(&"anything at all".to_string()).is_ok());
    }

    #[test]
    fn a_satisfied_contract_returns_ok() {
        let contract = non_empty();
        assert!(contract.check_pre(&"question".to_string()).is_ok());
        assert!(contract.check_post(&"short".to_string()).is_ok());
    }

    #[test]
    fn a_violated_precondition_names_the_invariant_that_failed() {
        let err = non_empty().check_pre(&"   ".to_string()).unwrap_err();
        assert_eq!(
            err,
            TraceErr::ContractViolated {
                message: "precondition: input must not be blank".to_string()
            }
        );
    }

    #[test]
    fn a_violated_postcondition_is_distinguishable_from_a_tool_failure() {
        let err = non_empty()
            .check_post(&"far too long to pass".to_string())
            .unwrap_err();
        assert!(matches!(err, TraceErr::ContractViolated { .. }));
        assert!(!matches!(err, TraceErr::ToolFailed { .. }));
        assert_eq!(
            err.to_string(),
            "contract violated: postcondition: output must be at most 10 chars"
        );
    }

    #[test]
    fn contract_step_records_the_violation_message_on_the_step() {
        let contract = non_empty();
        let failed = contract_step("len", &contract.check_post(&"x".repeat(20)));
        assert!(failed.is_failed());
        assert_eq!(
            failed.outcome,
            trace_lang_core::StepOutcome::Failed {
                message: "contract violated: postcondition: output must be at most 10 chars"
                    .to_string()
            }
        );

        let passed = contract_step("len", &contract.check_post(&"ok".to_string()));
        assert!(!passed.is_failed());
        assert_eq!(passed.name, "len");
    }

    #[test]
    fn a_later_declaration_replaces_an_earlier_one() {
        let contract: Contract<(), u32> = Contract::new()
            .post(|_: &u32| Err("first".to_string()))
            .post(|_: &u32| Err("second".to_string()));
        assert_eq!(
            contract.check_post(&1).unwrap_err().to_string(),
            "contract violated: postcondition: second"
        );
    }

    #[test]
    fn a_contract_is_send_and_sync_so_it_can_live_inside_an_agent() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Contract<String, String>>();
    }
}
