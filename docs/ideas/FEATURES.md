# proposed features

Eight design proposals for trace::, grouped by theme. None of these are implemented yet —
this document exists so we can argue about design before writing code. Each entry covers the
problem, a design sketch, how it integrates with what's already built (`trace-core`,
`trace-agent`, `trace-runtime`, `trace-task`), why it's differentiated from existing agentic
frameworks, and the open questions worth resolving before implementation starts.

---

## 1. `Trace<T, Trust>` — type-level provenance

### the problem

Right now, `Trace<T>` tells you *how* a value was produced — the causal chain, the branches
considered, the confidence at each step — but nothing stops you from taking a low-trust value
(an LLM's first-draft guess) and feeding it straight into a function that assumes high-trust
input (a publish step, a financial calculation, a database write). The information needed to
prevent that mistake exists in the trace at runtime, but nothing enforces it. A caller has to
remember to check `trace.low_confidence()` before using the value — and "remember to check"
is exactly the kind of discipline that erodes under deadline pressure.

### design sketch

Add a phantom type parameter to `Trace` that encodes trust level at the type level:

```rust
pub struct Trace<T, P = Unverified> {
    value: Option<T>,
    error: Option<TraceErr>,
    steps: Vec<Step>,
    _provenance: PhantomData<P>,
}

pub struct LLMGenerated;
pub struct ToolComputed;
pub struct HumanVerified;
pub struct Unverified; // default — today's behavior, unchanged

impl<T> Trace<T, LLMGenerated> {
    /// The only way to get a `HumanVerified` trace: a human actually looked at it.
    pub fn verified_by(self, reviewer: &str) -> Trace<T, HumanVerified> { .. }
}
```

Functions that require a trust level say so in their signature:

```rust
fn publish(t: Trace<Report, HumanVerified>) -> Result<(), PublishErr> { .. }
```

Calling `publish(llm_trace)` where `llm_trace: Trace<Report, LLMGenerated>` is a compile
error, not a runtime check. The trust level upgrade path (`verified_by`) is the only
constructor for `HumanVerified`, so there's no way to fabricate trust without it actually
happening.

### integration

`Agent::Output` would need to declare its default provenance — most agents produce
`LLMGenerated` by default; agents backed by deterministic tools (parsers, calculators) could
produce `ToolComputed`. `spawn()`/`delegate()` in `trace-agent` stay generic over the trust
parameter, since they don't need to know about it — they just thread `P` through unchanged.

### why this is different

LangGraph and CrewAI track provenance as runtime metadata at best (a field on a message, a
tag in a log). Nothing in either framework's type system prevents an unverified LLM output
from reaching a step that assumes it's safe. Rust's type system is the actual enforcement
mechanism here — this isn't a convention, it's a compiler error.

### open questions

- Does every crate that touches `Trace<T>` need a second generic parameter now? That's real
  ergonomic cost for a benefit most call sites won't need. A type alias
  (`type UncheckedTrace<T> = Trace<T, Unverified>`) softens this but doesn't eliminate it.
- Should trust levels form a lattice (e.g. `HumanVerified` implies `ToolComputed` implies
  `Unverified`) so a function requiring `Unverified` accepts anything? That needs a trait
  bound design, not just concrete phantom types.

---

## 2. Confidence decay

### the problem

A step's confidence score is a snapshot: "I was 0.9 confident when I ran this." But
confidence is not a fact about the world forever — it's a fact about the world *at the moment
the step ran*. A cached agent result from three days ago, scored 0.9 at creation, might be
badly wrong today if the underlying data changed. Nothing in `Trace<T>` currently
distinguishes "confident and current" from "confident and stale."

### design sketch

Attach an optional decay function to a `Step`, and add a time-aware confidence query to
`Trace`:

```rust
pub struct DecayCurve {
    half_life: Duration,
}

impl Step {
    pub fn with_decay(mut self, half_life: Duration) -> Self { .. }
}

impl<T: Clone + Serialize> Trace<T> {
    /// Confidence of the lowest-confidence step, adjusted for staleness at `at`.
    pub fn confidence_at(&self, at: DateTime<Utc>) -> f64 { .. }
}
```

A step scored `0.9` with a one-hour half-life reads as roughly `0.9` immediately, `~0.45` an
hour later, `~0.22` two hours later — standard exponential decay. Steps with no decay curve
never decay (today's behavior, unchanged) — this is opt-in per step, not a global policy.

### integration

`low_confidence_below()` in `trace-core` gains a time-aware sibling,
`low_confidence_below_at(threshold, at)`. `TaskRegistry` could use this on load: a task
resumed from a checkpoint written two days ago might want to re-verify a step whose decayed
confidence has dropped below threshold before trusting it, rather than blindly resuming.

### why this is different

This maps directly onto the trust-decay concept from agent-governance patterns — but applied
to *confidence over wall-clock time* rather than trust between agents. Most agent frameworks
treat a cached or checkpointed result as either valid or invalid with no notion of gradual
staleness. This makes staleness a first-class, queryable number instead of an implicit
assumption that checkpoints don't go stale.

### open questions

- What's the right default half-life when a step doesn't specify one — infinite (never
  decays, today's behavior) or some sane default? Infinite is probably correct: decay should
  be something an agent author opts into deliberately for steps they know are time-sensitive
  (e.g. "current stock price"), not something silently applied everywhere.
- Should decay compound across a causal chain, or only apply per-step? Compounding is more
  correct but harder to reason about — a five-step chain where each step decays independently
  could produce a confidence_at() that's confusingly lower than any individual step's current
  value.

---

## 3. Step cost ledger

### the problem

Every `trace::` run already produces a detailed record of what happened. It does not
currently record what that run *cost* — tokens consumed, dollars spent. Cost tracking today
lives in a separate system (log scraping, `ccusage`-style tooling) that has to be correlated
back to the agent run after the fact. The trace is the natural place for this data to live,
since it's already keyed by step and already the thing you'd inspect to understand a run.

### design sketch

```rust
pub struct StepCost {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub dollars: Option<f64>,
}

impl Step {
    pub fn with_cost(mut self, cost: StepCost) -> Self { .. }
}

impl<T: Clone + Serialize> Trace<T> {
    pub fn total_cost(&self) -> StepCost { .. }        // sum across all steps
    pub fn priciest_steps(&self) -> Vec<&Step> { .. }  // sorted by cost descending
}
```

`StepCost` implements `Add` so `total_cost()` is a straightforward fold. `priciest_steps()`
mirrors the existing `bottlenecks()` method (sorted by duration) but sorted by cost instead —
same shape, different metric, so the API stays consistent with what's already there.

### integration

The `observe` construct in the language design (wrapping a tool call in a trace span) is the
natural place to populate `StepCost` automatically for LLM calls, since that's where the
provider's token-usage response would be available. `TaskRegistry::save()` checkpoints would
then carry cumulative cost per task, letting a long-running pipeline answer "how much has
this cost so far" without external tooling.

### why this is different

This makes cost queryable from the trace itself rather than requiring a separate log-scraping
pass to reconstruct spend per agent run. It's a small feature, but it closes a real gap
between the tracing infrastructure and the cost-optimization tooling that would otherwise
have to reconstruct this correlation after the fact.

### open questions

- Dollar cost requires knowing the provider's pricing at call time, which changes over time
  and varies by provider. Should `StepCost` store just token counts and compute dollars
  lazily against a pluggable pricing table, or store the computed dollar amount at record
  time? Lazy computation is more correct for historical analysis (pricing changes shouldn't
  rewrite old checkpoints) but requires carrying a pricing table around at query time.

---

## 4. Deterministic replay & trace diffing

### the problem

When an agent's behavior changes — a prompt tweak, a model upgrade, a new tool — there's
currently no structured way to answer "what actually changed" beyond manually comparing
outputs. The trace already contains the full causal chain of a run; replaying that chain
against a new agent version and diffing the two traces is a natural way to turn traces into
regression tests, but nothing does this today.

### design sketch

```rust
impl Step {
    pub fn with_seed(mut self, seed: u64) -> Self { .. }
}

impl<T: Clone + Serialize> Trace<T> {
    /// True if every step in the chain recorded a seed (or needs none —
    /// e.g. a deterministic tool call).
    pub fn is_reproducible(&self) -> bool { .. }
}

pub struct TraceDiff {
    pub added_steps: Vec<Step>,
    pub removed_steps: Vec<Step>,
    pub changed_confidence: Vec<(Step, f64, f64)>, // step, old, new
    pub value_changed: bool,
}

pub async fn replay<A: Agent>(
    agent: &A,
    original: &Trace<A::Output>,
    input: A::Input,
) -> (Trace<A::Output>, TraceDiff) { .. }
```

`replay()` re-runs `agent` against the same `input` that produced `original`, then diffs the
new causal chain against the old one. Steps that recorded a `seed` can, in principle, be
replayed exactly if the underlying provider supports deterministic sampling; steps that
didn't are re-run as normal and the diff shows where behavior drifted.

### integration

This would live in its own crate (`trace-replay`) rather than `trace-core`, since it depends
on `trace-agent`'s `Agent` trait and adds a diffing algorithm that has nothing to do with
`Trace<T>`'s core definition. `TaskRegistry` checkpoints are the natural source of "original"
traces to replay against — a checkpoint file already has everything `replay()` needs.

### why this is different

This is "time-travel debugging" applied to agents: diffing behavior across agent versions
using the trace itself as the test fixture, rather than standing up a separate eval harness
and hoping it exercises the same code paths the production trace did. The trace *is* the
regression test.

### open questions

- True determinism requires the underlying LLM provider to support seeded/deterministic
  sampling, which not all do. `is_reproducible()` needs to be honest about this — a trace
  with recorded seeds is only as reproducible as the provider allows, and the API should not
  imply a stronger guarantee than that.
- What counts as "changed" for diffing purposes? A value that's semantically equivalent but
  textually different (two different but equally valid summaries) would show up as
  `value_changed: true` even though nothing is actually wrong. Diffing text output
  meaningfully is a much harder problem than diffing structured step metadata.

---

## 5. Pausable, resumable traces (human-in-the-loop)

### the problem

`EscalationAction::Delegate` hands a task off to another *agent*. There's no equivalent for
handing a task off to a *human* — stopping execution entirely, serializing state, and waiting
for an explicit decision that might come minutes or days later. Today, building this would
mean bolting an external approval gate onto the pipeline: a separate queue, a separate
storage mechanism, a separate resume path that doesn't know anything about `Trace<T>` or
`AgentContext`.

### design sketch

Add a new escalation variant and a corresponding paused state:

```rust
pub enum EscalationAction {
    None,
    Delegate(String),
    Emit(TraceErr),
    RequireApproval(ApprovalRequest), // new
}

pub struct ApprovalRequest {
    pub question: String,
    pub context: serde_json::Value, // whatever the agent wants a human to see
}

pub enum TraceState<T> {
    Complete(Trace<T>),
    Paused {
        checkpoint: PausedCheckpoint<T>,
        request: ApprovalRequest,
    },
}

pub struct PausedCheckpoint<T> {
    pub partial_trace: Trace<T>,
    pub agent_name: String,
    pub resume_input: serde_json::Value, // serialized input to resume with
}

pub fn resume<T>(checkpoint: PausedCheckpoint<T>, decision: ApprovalDecision) -> TraceState<T> { .. }
```

The key design decision: this is not a delegation to a "HumanReviewer" agent (which is
already possible today via the existing `Delegate` mechanism and works fine for cases where
"human" is really just another named endpoint in the registry). This is a genuine
stop-the-world pause where the entire pipeline state is serialized and nothing runs again
until a decision arrives through some external channel — a Slack approval, a CLI prompt, a
web form days later.

### integration

`TaskRegistry` already checkpoints after every task transition
(`registry.save("./checkpoint.trace.json")`). A `Paused` task status would fit naturally next
to `Pending`/`Running`/`Done`/`Failed` — `TaskStatus::Paused(ApprovalRequest)` — and the
existing `save()`/`load()` round-trip already handles the serialization half of this for
free. This is the single biggest reason this feature belongs in `trace::` specifically: the
infrastructure it needs (serializable checkpoints, resumable task graphs) already exists for
other reasons.

### why this is different

Most agent frameworks treat human approval as something you build *around* the framework —
an external gate that pauses your orchestration code, not something the framework's core
types understand. Here, `Paused` is a variant of the pipeline's own state representation, so
every tool that already understands `TaskRegistry` checkpoints (inspection, resumption,
audit) understands paused tasks for free, without special-casing them.

### open questions

- How long can a checkpoint safely live before the world has moved on enough that resuming
  with stale context is actively wrong? This is where confidence decay (feature 2) would
  compose naturally — a paused checkpoint older than some threshold could require re-running
  earlier steps rather than blindly resuming.
- What's the resume API for the *human* side of this — a CLI, a web form, a Slack
  integration? The type design above is channel-agnostic on purpose, but a real
  implementation needs at least one concrete channel to be useful.

---

## 6. `contract!` step pre/post-conditions

### the problem

An agent step can produce output that's *technically* successful (no `TraceErr`, no
exception) but substantively wrong — an empty summary, a malformed identifier, a value
outside a valid range. Nothing currently catches this class of failure; it silently becomes
whatever downstream code does with a bad value, and by the time someone notices, the trace
that would explain what went wrong is long gone.

### design sketch

```rust
pub struct Contract<I, O> {
    pub pre: Option<Box<dyn Fn(&I) -> Result<(), String> + Send + Sync>>,
    pub post: Option<Box<dyn Fn(&O) -> Result<(), String> + Send + Sync>>,
}

// usage inside an Agent::run implementation
let contract = Contract::new()
    .post(|summary: &Summary| {
        if summary.text.is_empty() {
            Err("summary must not be empty".to_string())
        } else {
            Ok(())
        }
    });

let result = contract.check_post(&output)?; // returns TraceErr on violation
```

A contract violation produces a `Step` with `StepOutcome::Failed { message }` where the
message is the specific contract that failed — not a generic error, but "postcondition
violated: summary must not be empty." This is what makes it useful for debugging: the
violated invariant is recorded, not just the fact that something went wrong.

### integration

This sits naturally inside `Agent::run` implementations rather than as a separate crate — the
contract is defined alongside the step logic it checks, so `Contract<I, O>` would likely live
in `trace-agent` next to `Agent` itself, since it's specifically about constraining a step's
input/output rather than a general-purpose validation library.

### why this is different

This is design-by-contract applied specifically to agentic steps, where the contract
violation becomes a first-class, queryable trace event rather than a bug report someone files
after noticing bad output downstream. The trace explains not just that a step produced bad
output, but exactly which invariant it violated and when — turning "the summarizer sometimes
returns garbage" into a specific, searchable failure mode.

### open questions

- Contracts add real overhead if checked on every step of a hot loop. Should contract
  checking be opt-in per-agent (a `budget`-style declaration) or always-on with a way to
  disable it for performance-sensitive agents?
- Should a contract violation be its own `TraceErr` variant (`ContractViolated { message }`)
  rather than a generic `Failed`? A dedicated variant would let `on_step_failure` hooks
  distinguish "the tool call itself failed" from "the tool succeeded but violated its
  contract" — probably worth doing, since the appropriate escalation differs (retry vs.
  escalate to a human who understands the domain invariant).

---

## 7. `TraceGraph` — cross-trace lineage

### the problem

`Trace::causal_chain()` explains one agent's run in isolation. It does not explain how that
run relates to the five other traces that fed into it, or the three traces downstream that
consumed its output. In a real pipeline — `Decomposer` produces tasks, `Executor` runs them,
each task's completion links to a `TraceRef` — there's no way today to ask "show me
everything downstream of this one trace" without manually walking `TaskRegistry` and cross-
referencing `TraceRef`s by hand.

### design sketch

```rust
pub struct TraceGraph {
    nodes: HashMap<TraceRef, TraceNode>,
    edges: Vec<(TraceRef, TraceRef)>, // (producer, consumer)
}

impl TraceGraph {
    pub fn record_edge(&mut self, producer: TraceRef, consumer: TraceRef) { .. }
    pub fn downstream_of(&self, t: &TraceRef) -> Vec<&TraceNode> { .. }
    pub fn upstream_of(&self, t: &TraceRef) -> Vec<&TraceNode> { .. }
    /// The longest chain of dependent traces — useful for spotting where
    /// a pipeline's latency actually comes from.
    pub fn critical_path(&self) -> Vec<TraceRef> { .. }
}
```

An edge is recorded whenever one agent's output (identified by its `TraceRef`) becomes
another agent's input. In practice this means `spawn()`/`delegate()` would need an optional
"this input came from trace X" annotation, or the calling code populates the graph
explicitly after the fact from `TaskRegistry`'s existing `depends_on` edges.

### integration

`TaskRegistry` already has a dependency graph — `Task::depends_on: Vec<Uuid>` — at the task
level. `TraceGraph` is the same idea one level down, at the trace level: tasks can depend on
other tasks without necessarily needing every trace-to-trace edge recorded, but when you do
need "which specific agent output caused this failure three hops upstream," `TraceGraph` is
what answers that question that `TaskRegistry` alone can't.

### why this is different

Most agentic frameworks give you observability into a single chain or a single agent's
internal steps, not a queryable DAG across an entire multi-agent pipeline's history. This
extends the dependency-graph idea that's already load-bearing in `trace-task` from the task
level down to the trace level, so "why did this fail" can be answered by walking backward
through actual causal edges instead of manually correlating timestamps across log files.

### open questions

- At what granularity do edges get recorded — automatically by the runtime (accurate but
  requires threading trace identity through every function call), or manually by agent
  authors (simpler to implement but relies on discipline to actually call `record_edge`)?
  Automatic is clearly better but is a bigger change to `trace-agent`'s core APIs.
- Does `TraceGraph` need its own persistence story, or does it live entirely in-memory for
  the duration of a pipeline run and get reconstructed from `TaskRegistry` checkpoints when
  needed? In-memory is simpler; reconstructing from checkpoints avoids yet another
  serialized-state file to keep in sync.

---

## 8. `speculate_race` — early-exit speculative branching

### the problem

The existing `speculate()` combinator (in `trace-runtime`) runs every candidate agent to
completion before picking a winner by confidence. That's correct when you genuinely want to
compare every option, but it's wasteful when one candidate clears an acceptable confidence
bar quickly — you've paid the full latency and cost of every losing branch for no benefit
once a "good enough" answer exists.

### design sketch

```rust
pub async fn speculate_race<I, O>(
    candidates: Vec<(String, Arc<dyn Agent<Input = I, Output = O>>)>,
    input: I,
    threshold: f64,
) -> Trace<O>
where
    I: Clone + Send,
    O: Clone + Serialize + Send,
{
    // race all candidates; as soon as one crosses `threshold`, cancel the rest
    // and return immediately. if none cross the threshold before all finish,
    // fall back to speculate()'s existing highest-confidence-wins behavior.
}
```

Implementation-wise this likely uses `tokio::select!` in a loop over a `FuturesUnordered` set
rather than `futures::future::join_all` (which waits for everything) — as each candidate
completes, check its confidence; if it clears `threshold`, drop the remaining futures
(cancelling them) and return immediately with that candidate's trace, still recording a
`speculate` step with `Branch`es for whichever candidates did get a chance to finish before
cancellation.

### integration

This is an additive sibling to `speculate()` in `trace-runtime`, not a replacement — some
call sites genuinely want the "compare everything" semantics of the original, others want the
latency/cost savings of racing to an acceptable answer. Both belong in the same module and
share the confidence-scoring and branch-recording logic that `speculate()` already has.

### why this is different

Most speculative-execution patterns in agent frameworks are all-or-nothing: either you run
one candidate, or you run all of them and wait. This makes the accuracy/latency/cost tradeoff
a tunable parameter of the combinator itself (`threshold`) rather than a binary choice between
two different APIs — call `speculate_race` with a high threshold and it behaves almost like
`speculate`; call it with a low threshold and it behaves almost like "take the first
plausible answer."

### open questions

- Cancelling in-flight futures via `tokio::select!`/`FuturesUnordered` means cancelled
  candidates never get to record their own confidence — their `Branch` entry would have to
  say "cancelled" rather than "rejected: lower confidence," which is a meaningfully different
  and arguably more honest signal. `Branch`/`BranchOutcome` in `trace-core` would need a third
  outcome variant to represent this accurately rather than overloading `Rejected`.
- What threshold is "acceptable" by default, and should it be able to vary per-candidate
  (some agents are known to be more reliable and could race with a lower bar) rather than one
  global threshold for every candidate in the set?

---

## prioritization notes

Of these eight, **pausable/resumable traces** (#5) is the most structurally novel — it's the
one idea here that would be load-bearing on infrastructure this repo already built
(`TaskRegistry`'s checkpoint/resume story) rather than requiring new infrastructure from
scratch. **Step cost ledger** (#3) is the lowest-effort, highest-immediate-value addition —
it's a small, additive change to `Step` with no new crate and no design controversy. The
remaining six are all reasonable next steps but involve more open design questions than
implementation effort, which is why this document exists before any of them get written.
