//! End-to-end smoke driver for the tracers workspace.
//!
//! Exercises all four crates against their public API, the way a downstream
//! consumer would: build a Trace, run agents via spawn/join_all/speculate,
//! resolve a Delegate escalation through AgentRegistry, and round-trip a
//! TaskRegistry through FileCheckpointStore. Exits non-zero on any failure.

use std::sync::Arc;

use async_trait::async_trait;
use trace_lang_agent::{Agent, AgentContext, spawn};
use trace_lang_core::{Step, Trace};
use trace_lang_runtime::{AgentRegistry, join_all, run_with_escalation, speculate};
use trace_lang_task::{FileCheckpointStore, Priority, Task, TaskRegistry};

struct Greeter {
    name: &'static str,
    confidence: f64,
}

#[async_trait]
impl Agent for Greeter {
    type Input = String;
    type Output = String;

    fn name(&self) -> &str {
        self.name
    }
    fn goal(&self) -> &str {
        "produce a greeting"
    }

    async fn run(&self, input: Self::Input, ctx: &mut AgentContext) -> Trace<Self::Output> {
        if ctx.record_step().is_err() {
            return Trace::failed(trace_lang_core::TraceErr::other("budget exceeded"));
        }
        let mut t = Trace::new(format!("hello, {input}! (from {})", self.name));
        t.push_step(Step::named("greet").with_confidence(self.confidence));
        t
    }
}

#[tokio::main]
async fn main() {
    // 1. core: Trace as a value with provenance
    let mut trace = Trace::new(42);
    trace.push_step(Step::named("compute").with_confidence(0.9));
    assert_eq!(trace.value(), Some(&42));
    assert_eq!(trace.causal_chain().len(), 1);
    println!("core: Trace ok — {} step(s)", trace.causal_chain().len());

    // 2. agent: spawn a single agent
    let alice = Greeter { name: "Alice", confidence: 0.95 };
    let outcome = spawn(&alice, "world".to_string()).await;
    assert!(outcome.trace.is_ok());
    println!("agent: spawn ok — {:?}", outcome.trace.value().unwrap());

    // 3. runtime: fan out over inputs
    let results = join_all(&alice, vec!["a".into(), "b".into(), "c".into()]).await;
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|o| o.trace.is_ok()));
    println!("runtime: join_all ok — {} results", results.len());

    // 4. runtime: speculative branching, winner by confidence
    let candidates: Vec<(String, Arc<dyn Agent<Input = String, Output = String>>)> = vec![
        ("timid".into(), Arc::new(Greeter { name: "Timid", confidence: 0.3 })),
        ("bold".into(), Arc::new(Greeter { name: "Bold", confidence: 0.99 })),
    ];
    let winner = speculate(candidates, "race".to_string()).await;
    let val = winner.value().expect("speculate produced a value").clone();
    assert!(val.contains("Bold"), "expected Bold to win, got {val}");
    println!("runtime: speculate ok — winner: {val:?}");

    // 5. runtime: registry + escalation resolution (0 hops needed here)
    let mut registry = AgentRegistry::new();
    registry.register(Arc::new(Greeter { name: "Senior", confidence: 0.9 })
        as Arc<dyn Agent<Input = String, Output = String>>);
    let junior = Greeter { name: "Junior", confidence: 0.9 };
    let run = run_with_escalation(&junior, "task".to_string(), &registry, 2).await;
    assert!(run.trace.is_ok());
    println!("runtime: run_with_escalation ok");

    // 6. task: registry with dependency graph, checkpointed to disk
    let dir = std::env::temp_dir().join("tracers-smoke");
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    let ckpt = dir.join("registry.json");
    let store = FileCheckpointStore::new(&ckpt);

    let mut tasks = TaskRegistry::new();
    let t1 = Task::new("fetch requirements").with_priority(Priority::High);
    let t1_id = t1.id;
    let t2 = Task::new("plan architecture").depends_on(t1_id);
    tasks.insert(t1);
    tasks.insert(t2);
    assert_eq!(tasks.ready_tasks().len(), 1);

    tasks
        .complete(t1_id, outcome.trace.trace_ref(), &store)
        .expect("complete + checkpoint");
    assert_eq!(tasks.done().len(), 1);
    assert_eq!(tasks.ready_tasks().len(), 1); // t2 unblocked

    // 7. task: crash recovery — reload from the checkpoint file
    let restored = TaskRegistry::load(&store).expect("load checkpoint");
    assert_eq!(restored.total(), 2);
    assert_eq!(restored.done().len(), 1);
    println!("task: checkpoint round-trip ok — {}", ckpt.display());

    println!("SMOKE OK: all 7 checks passed");
}
