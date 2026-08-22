//! Cross-trace lineage: `Trace::causal_chain()` explains one run,
//! `TraceGraph` explains how runs caused each other.
//!
//! ```text
//! cargo run -p trace-lang-examples --example trace_graph
//! ```

use std::time::Duration;

use trace_lang_core::{Step, Trace, TraceGraph, TraceNode, TraceRef};
use trace_lang_examples::{fact, heading};

/// Stand in for an agent run: a trace with one timed step.
fn run(label: &str, millis: u64) -> Trace<String> {
    let mut trace = Trace::new(format!("{label} output"));
    trace.push_step(
        Step::named(label)
            .with_confidence(0.9)
            .with_duration(Duration::from_millis(millis)),
    );
    trace
}

fn main() {
    // A four-agent pipeline that forks and rejoins:
    //
    //   fetch ──► parse ──► summarize ──► publish
    //         └─► lint ─────────────────► publish
    let fetch = run("fetch", 120);
    let parse = run("parse", 40);
    let lint = run("lint", 900);
    let summarize = run("summarize", 2_600);
    let publish = run("publish", 80);

    let mut graph = TraceGraph::new();
    for (trace, label) in [
        (&fetch, "Fetcher"),
        (&parse, "Parser"),
        (&lint, "Linter"),
        (&summarize, "Summarizer"),
        (&publish, "Publisher"),
    ] {
        graph.record_node(TraceNode::from_trace(trace, label));
    }

    // An edge says "this trace's output became that trace's input".
    graph.record_edge(fetch.trace_ref(), parse.trace_ref());
    graph.record_edge(fetch.trace_ref(), lint.trace_ref());
    graph.record_edge(parse.trace_ref(), summarize.trace_ref());
    graph.record_edge(summarize.trace_ref(), publish.trace_ref());
    graph.record_edge(lint.trace_ref(), publish.trace_ref());

    heading("the graph");
    fact("traces", graph.len());
    fact("edges", graph.edges().len());

    heading("everything the fetch caused");
    // Transitive, not just direct consumers — this is the "what did this
    // one bad result contaminate" question.
    for node in graph.downstream_of(&fetch.trace_ref()) {
        fact(&node.label, describe(node));
    }

    heading("everything that fed the publish");
    // Run this on a failed trace and you have "why did this fail, three
    // hops upstream" without correlating timestamps across log files.
    for node in graph.upstream_of(&publish.trace_ref()) {
        fact(&node.label, describe(node));
    }

    heading("where the latency actually comes from");
    // Not the longest chain — the slowest one. `lint` is a dead end at
    // 900ms; the parse → summarize path costs far more.
    let path = graph.critical_path();
    let total: Duration = path
        .iter()
        .filter_map(|r| graph.node(r).and_then(|n| n.duration))
        .sum();
    fact(
        "critical path",
        path.iter()
            .map(|r| label_of(&graph, r))
            .collect::<Vec<_>>()
            .join(" → "),
    );
    fact("total on that path", format!("{total:?}"));

    heading("the graph is a value, like everything else");
    let json = serde_json::to_string(&graph).expect("a TraceGraph serializes");
    fact("serialized bytes", json.len());
    let restored: TraceGraph = serde_json::from_str(&json).expect("and deserializes");
    fact("restored traces", restored.len());
    fact(
        "same critical path",
        restored.critical_path() == graph.critical_path(),
    );
}

fn describe(node: &TraceNode) -> String {
    match node.duration {
        Some(d) => format!("{d:?}"),
        None => "untimed".to_string(),
    }
}

fn label_of(graph: &TraceGraph, trace_ref: &TraceRef) -> String {
    graph
        .node(trace_ref)
        .map(|n| n.label.clone())
        .unwrap_or_else(|| trace_ref.to_string())
}
