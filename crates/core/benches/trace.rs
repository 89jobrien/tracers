//! Benchmarks for the operations a real pipeline does per step, and for the
//! ones it does once per inspection.
//!
//! ```bash
//! cargo bench -p trace-lang-core
//! ```
//!
//! The interesting question here is not raw speed — it is whether carrying
//! provenance as a *value* costs anything meaningful next to the LLM call it
//! is describing. Everything below runs against traces of 10 to 1,000 steps,
//! which brackets the range a long agent run actually produces.

use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use trace_lang_core::{Step, StepCost, Trace, TraceGraph, TraceNode, TraceRef};
use uuid::Uuid;

const SIZES: [usize; 3] = [10, 100, 1_000];

/// A trace of `steps` steps, every one carrying confidence, duration, and cost
/// — the worst case for the query benchmarks below.
fn populated(steps: usize) -> Trace<String> {
    let mut trace = Trace::new("result".to_string());
    for i in 0..steps {
        trace.push_step(
            Step::named(format!("step-{i}"))
                .with_confidence((i % 100) as f64 / 100.0)
                .with_duration(Duration::from_micros(i as u64))
                .with_cost(StepCost::new(i as u64, i as u64 / 2).with_dollars(i as f64 / 1_000.0)),
        );
    }
    trace
}

/// The per-step cost: what an agent pays on every unit of work.
fn bench_push_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("push_step");
    group.bench_function("one_step", |b| {
        let mut trace = Trace::new(1u32);
        b.iter(|| trace.push_step(black_box(Step::named("work").with_confidence(0.9))));
    });
    group.bench_function("one_step_with_cost", |b| {
        let mut trace = Trace::new(1u32);
        b.iter(|| {
            trace.push_step(black_box(
                Step::named("work")
                    .with_confidence(0.9)
                    .with_cost(StepCost::new(1_000, 200).with_dollars(0.004)),
            ))
        });
    });
    group.finish();
}

/// The inspection cost: what a caller pays to ask the trace a question.
fn bench_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("queries");
    for size in SIZES {
        let trace = populated(size);
        group.bench_with_input(BenchmarkId::new("causal_chain", size), &trace, |b, t| {
            b.iter(|| black_box(t.causal_chain().len()))
        });
        group.bench_with_input(BenchmarkId::new("bottlenecks", size), &trace, |b, t| {
            b.iter(|| black_box(t.bottlenecks().len()))
        });
        group.bench_with_input(BenchmarkId::new("low_confidence", size), &trace, |b, t| {
            b.iter(|| black_box(t.low_confidence().len()))
        });
        group.bench_with_input(BenchmarkId::new("total_cost", size), &trace, |b, t| {
            b.iter(|| black_box(t.total_cost()))
        });
        group.bench_with_input(BenchmarkId::new("priciest_steps", size), &trace, |b, t| {
            b.iter(|| black_box(t.priciest_steps().len()))
        });
    }
    group.finish();
}

/// Checkpointing is on the hot path — `TaskRegistry::save()` runs after every
/// state transition, so serialization cost is paid constantly, not rarely.
fn bench_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("serde");
    for size in SIZES {
        let trace = populated(size);
        let json = serde_json::to_string(&trace).expect("a trace serializes");

        group.bench_with_input(BenchmarkId::new("serialize", size), &trace, |b, t| {
            b.iter(|| black_box(serde_json::to_string(t).expect("serializes")))
        });
        group.bench_with_input(BenchmarkId::new("deserialize", size), &json, |b, j| {
            b.iter(|| {
                black_box(serde_json::from_str::<Trace<String>>(j).expect("deserializes"));
            })
        });
    }
    group.finish();
}

/// `critical_path` is the only non-linear thing in the crate — a memoized DFS
/// over the whole graph — so it is the one worth watching for regressions.
fn bench_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph");
    for size in [10usize, 100, 500] {
        let refs: Vec<TraceRef> = (0..size).map(|_| TraceRef(Uuid::new_v4())).collect();
        let mut graph = TraceGraph::new();
        for (i, node) in refs.iter().enumerate() {
            graph.record_node(
                TraceNode::new(node.clone(), format!("agent-{i}"))
                    .with_duration(Duration::from_millis(i as u64)),
            );
        }
        // A chain with a shortcut every fourth node, so the search has real
        // branching to resolve rather than one obvious path.
        for i in 1..size {
            graph.record_edge(refs[i - 1].clone(), refs[i].clone());
            if i >= 4 {
                graph.record_edge(refs[i - 4].clone(), refs[i].clone());
            }
        }

        group.bench_with_input(BenchmarkId::new("critical_path", size), &graph, |b, g| {
            b.iter(|| black_box(g.critical_path().len()))
        });
        group.bench_with_input(BenchmarkId::new("downstream_of", size), &graph, |b, g| {
            b.iter(|| black_box(g.downstream_of(&refs[0]).len()))
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_push_step,
    bench_queries,
    bench_serde,
    bench_graph
);
criterion_main!(benches);
