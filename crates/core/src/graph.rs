use crate::trace::{Trace, TraceRef};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

/// One trace's entry in a [`TraceGraph`].
///
/// A node is deliberately thin: it carries identity, a human-readable
/// label, and the wall time the trace took. The full `Trace<T>` is not
/// stored, because `TraceGraph` is generic over *no* value type — a real
/// pipeline's traces carry different `T`s, and lineage is a question about
/// their relationships, not their payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceNode {
    pub trace_ref: TraceRef,
    /// Usually the producing agent's name. Empty for nodes created
    /// implicitly by [`TraceGraph::record_edge`].
    pub label: String,
    /// Total wall time, used to weight [`TraceGraph::critical_path`].
    pub duration: Option<Duration>,
}

impl TraceNode {
    /// A node for `trace_ref`, labelled but not yet timed.
    pub fn new(trace_ref: TraceRef, label: impl Into<String>) -> Self {
        Self {
            trace_ref,
            label: label.into(),
            duration: None,
        }
    }

    /// Builder: record how long the trace took.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// A node for `trace`, timed by summing its steps' recorded durations.
    /// Steps with no duration contribute zero.
    pub fn from_trace<T: Clone + serde::Serialize>(
        trace: &Trace<T>,
        label: impl Into<String>,
    ) -> Self {
        let duration = trace
            .causal_chain()
            .iter()
            .filter_map(|s| s.duration)
            .sum::<Duration>();
        Self::new(trace.trace_ref(), label).with_duration(duration)
    }
}

/// A directed acyclic graph of trace-to-trace lineage.
///
/// `Trace::causal_chain()` explains one run. `TraceGraph` explains how
/// runs relate: which trace produced the input another trace consumed.
/// It is the same idea as `Task::depends_on` in `trace-lang-task`, one
/// level down — tasks depend on tasks, traces are caused by traces — and
/// answers the question a task graph alone cannot: *which specific agent
/// output caused this failure three hops upstream*.
///
/// Edges are recorded explicitly by the caller (`record_edge`), not
/// inferred: nothing in `spawn`/`delegate` currently threads producer
/// identity through, so an automatically-populated graph would be
/// silently incomplete rather than merely empty.
///
/// ```rust
/// use trace_lang_core::{Trace, TraceGraph, TraceNode};
///
/// let fetch = Trace::new("raw data");
/// let summarize = Trace::new("summary");
///
/// let mut graph = TraceGraph::new();
/// graph.record_node(TraceNode::from_trace(&fetch, "Fetcher"));
/// graph.record_node(TraceNode::from_trace(&summarize, "Summarizer"));
/// graph.record_edge(fetch.trace_ref(), summarize.trace_ref());
///
/// let downstream = graph.downstream_of(&fetch.trace_ref());
/// assert_eq!(downstream.len(), 1);
/// assert_eq!(downstream[0].label, "Summarizer");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceGraph {
    nodes: HashMap<TraceRef, TraceNode>,
    /// `(producer, consumer)` pairs, in the order they were recorded.
    edges: Vec<(TraceRef, TraceRef)>,
}

impl TraceGraph {
    /// Construct an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    // ── Mutation ──────────────────────────────────────────────────────────────

    /// Insert `node`, replacing any existing node with the same `TraceRef`.
    pub fn record_node(&mut self, node: TraceNode) {
        self.nodes.insert(node.trace_ref.clone(), node);
    }

    /// Record that `producer`'s output became `consumer`'s input.
    ///
    /// Either endpoint that has no node yet gets an unlabelled placeholder,
    /// so an edge is never dangling — call [`Self::record_node`] before or
    /// after to fill in the label and duration.
    ///
    /// Self-edges and exact duplicates are ignored: a trace cannot cause
    /// itself, and recording the same lineage twice is not two edges.
    pub fn record_edge(&mut self, producer: TraceRef, consumer: TraceRef) {
        if producer == consumer {
            return;
        }
        let edge = (producer.clone(), consumer.clone());
        if self.edges.contains(&edge) {
            return;
        }
        for r in [&producer, &consumer] {
            self.nodes
                .entry(r.clone())
                .or_insert_with(|| TraceNode::new(r.clone(), ""));
        }
        self.edges.push(edge);
    }

    // ── Querying ──────────────────────────────────────────────────────────────

    /// Borrow a node by reference, if it is in the graph.
    pub fn node(&self, trace_ref: &TraceRef) -> Option<&TraceNode> {
        self.nodes.get(trace_ref)
    }

    /// Every recorded edge, in insertion order.
    pub fn edges(&self) -> &[(TraceRef, TraceRef)] {
        &self.edges
    }

    /// Number of nodes in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True if no nodes have been recorded.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Every trace transitively caused by `trace_ref`, breadth-first.
    /// Excludes `trace_ref` itself.
    pub fn downstream_of(&self, trace_ref: &TraceRef) -> Vec<&TraceNode> {
        self.reachable(trace_ref, Direction::Forward)
    }

    /// Every trace that transitively fed into `trace_ref`, breadth-first.
    /// Excludes `trace_ref` itself.
    pub fn upstream_of(&self, trace_ref: &TraceRef) -> Vec<&TraceNode> {
        self.reachable(trace_ref, Direction::Backward)
    }

    /// The chain of dependent traces that dominates the pipeline's latency.
    ///
    /// Paths are ranked by summed node duration, tie-broken by hop count —
    /// so a graph where nothing recorded a duration still returns the
    /// longest causal chain rather than an arbitrary single node. Returns
    /// producer-first, and an empty `Vec` for an empty graph.
    ///
    /// Edges run producer → consumer, so a cycle should be impossible; if
    /// one is present anyway the back edge is ignored rather than looped on.
    pub fn critical_path(&self) -> Vec<TraceRef> {
        // best[n] = the highest-ranked path *starting* at n, as
        // (summed duration, hop count, the path itself).
        let mut best: HashMap<TraceRef, (Duration, usize, Vec<TraceRef>)> = HashMap::new();
        let mut in_progress: HashSet<TraceRef> = HashSet::new();

        for start in self.sorted_refs() {
            self.longest_from(&start, &mut best, &mut in_progress);
        }

        best_of(self.sorted_refs().into_iter().filter_map(|r| best.get(&r)))
            .map(|(_, _, path)| path.clone())
            .unwrap_or_default()
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Node references in a deterministic order — `HashMap` iteration order
    /// is not stable across runs, and `critical_path` must be.
    fn sorted_refs(&self) -> Vec<TraceRef> {
        let mut refs: Vec<TraceRef> = self.nodes.keys().cloned().collect();
        refs.sort_by_key(|r| r.0);
        refs
    }

    fn neighbors(&self, of: &TraceRef, direction: Direction) -> Vec<TraceRef> {
        self.edges
            .iter()
            .filter_map(|(producer, consumer)| match direction {
                Direction::Forward if producer == of => Some(consumer.clone()),
                Direction::Backward if consumer == of => Some(producer.clone()),
                _ => None,
            })
            .collect()
    }

    fn reachable(&self, from: &TraceRef, direction: Direction) -> Vec<&TraceNode> {
        let mut seen: HashSet<TraceRef> = HashSet::from([from.clone()]);
        let mut queue: VecDeque<TraceRef> = VecDeque::from([from.clone()]);
        let mut out = Vec::new();

        while let Some(current) = queue.pop_front() {
            for next in self.neighbors(&current, direction) {
                if !seen.insert(next.clone()) {
                    continue;
                }
                if let Some(node) = self.nodes.get(&next) {
                    out.push(node);
                }
                queue.push_back(next);
            }
        }
        out
    }

    /// Iterative memoized DFS for the best path starting at `start`.
    /// Iterative rather than recursive so a pathological graph exhausts a
    /// `Vec`, not the call stack.
    fn longest_from(
        &self,
        start: &TraceRef,
        best: &mut HashMap<TraceRef, (Duration, usize, Vec<TraceRef>)>,
        in_progress: &mut HashSet<TraceRef>,
    ) {
        let mut stack: Vec<(TraceRef, bool)> = vec![(start.clone(), false)];

        while let Some((current, expanded)) = stack.pop() {
            if best.contains_key(&current) {
                continue;
            }

            if !expanded {
                if !in_progress.insert(current.clone()) {
                    continue; // back edge — ignore rather than loop
                }
                stack.push((current.clone(), true));
                for next in self.neighbors(&current, Direction::Forward) {
                    if !best.contains_key(&next) && !in_progress.contains(&next) {
                        stack.push((next, false));
                    }
                }
                continue;
            }

            let neighbors = self.neighbors(&current, Direction::Forward);
            let tail = best_of(neighbors.iter().filter_map(|next| best.get(next)))
                .cloned()
                .unwrap_or((Duration::ZERO, 0, Vec::new()));

            let own = self
                .nodes
                .get(&current)
                .and_then(|n| n.duration)
                .unwrap_or(Duration::ZERO);

            let mut path = Vec::with_capacity(tail.2.len() + 1);
            path.push(current.clone());
            path.extend(tail.2);

            in_progress.remove(&current);
            best.insert(current, (own.saturating_add(tail.0), tail.1 + 1, path));
        }
    }
}

/// Pick the highest-ranked candidate path — slowest first, ties broken by
/// hop count, and remaining ties kept **first**.
///
/// A manual fold rather than `Iterator::max_by` for the same reason
/// `speculate`'s winner selection is: `max_by` returns the *last* maximum on
/// a tie, which would make the reported critical path depend on `TraceRef`
/// UUID ordering rather than on recording order.
fn best_of<'a>(
    candidates: impl Iterator<Item = &'a (Duration, usize, Vec<TraceRef>)>,
) -> Option<&'a (Duration, usize, Vec<TraceRef>)> {
    let mut best: Option<&(Duration, usize, Vec<TraceRef>)> = None;
    for candidate in candidates {
        let better = match best {
            None => true,
            Some(current) => (candidate.0, candidate.1) > (current.0, current.1),
        };
        if better {
            best = Some(candidate);
        }
    }
    best
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Forward,
    Backward,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::Step;
    use uuid::Uuid;

    fn r() -> TraceRef {
        TraceRef(Uuid::new_v4())
    }

    /// a -> b -> c, with a side branch a -> d.
    fn diamondless_chain() -> (TraceGraph, TraceRef, TraceRef, TraceRef, TraceRef) {
        let (a, b, c, d) = (r(), r(), r(), r());
        let mut g = TraceGraph::new();
        for (node, label) in [(&a, "A"), (&b, "B"), (&c, "C"), (&d, "D")] {
            g.record_node(TraceNode::new(node.clone(), label));
        }
        g.record_edge(a.clone(), b.clone());
        g.record_edge(b.clone(), c.clone());
        g.record_edge(a.clone(), d.clone());
        (g, a, b, c, d)
    }

    #[test]
    fn record_edge_creates_placeholder_nodes_for_unknown_refs() {
        let (a, b) = (r(), r());
        let mut g = TraceGraph::new();
        g.record_edge(a.clone(), b.clone());

        assert_eq!(g.len(), 2);
        assert_eq!(g.node(&a).unwrap().label, "");
        assert_eq!(g.edges().len(), 1);
    }

    #[test]
    fn record_edge_ignores_self_edges_and_duplicates() {
        let (a, b) = (r(), r());
        let mut g = TraceGraph::new();
        g.record_edge(a.clone(), a.clone());
        assert!(g.is_empty());

        g.record_edge(a.clone(), b.clone());
        g.record_edge(a.clone(), b.clone());
        assert_eq!(g.edges().len(), 1);
    }

    #[test]
    fn record_node_overwrites_a_placeholder_without_dropping_edges() {
        let (a, b) = (r(), r());
        let mut g = TraceGraph::new();
        g.record_edge(a.clone(), b.clone());
        g.record_node(TraceNode::new(a.clone(), "Fetcher"));

        assert_eq!(g.node(&a).unwrap().label, "Fetcher");
        assert_eq!(g.edges().len(), 1);
    }

    #[test]
    fn downstream_of_is_transitive_and_excludes_the_start() {
        let (g, a, b, c, d) = diamondless_chain();
        let labels: Vec<&str> = g
            .downstream_of(&a)
            .iter()
            .map(|n| n.label.as_str())
            .collect();
        assert_eq!(labels, vec!["B", "D", "C"]); // breadth-first
        assert_eq!(g.downstream_of(&c).len(), 0);
        assert_eq!(g.downstream_of(&b)[0].label, "C");
        assert_eq!(g.downstream_of(&d).len(), 0);
    }

    #[test]
    fn upstream_of_walks_edges_backwards() {
        let (g, a, b, c, _d) = diamondless_chain();
        let labels: Vec<&str> = g.upstream_of(&c).iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, vec!["B", "A"]);
        assert_eq!(g.upstream_of(&a).len(), 0);
        assert_eq!(g.upstream_of(&b)[0].label, "A");
    }

    #[test]
    fn critical_path_picks_the_slowest_chain_not_the_longest_one() {
        let (a, slow, fast, tail) = (r(), r(), r(), r());
        let mut g = TraceGraph::new();
        g.record_node(TraceNode::new(a.clone(), "A"));
        g.record_node(TraceNode::new(slow.clone(), "Slow").with_duration(Duration::from_secs(10)));
        g.record_node(TraceNode::new(fast.clone(), "Fast").with_duration(Duration::from_millis(1)));
        g.record_node(TraceNode::new(tail.clone(), "Tail").with_duration(Duration::from_millis(1)));

        // A -> Slow  (2 hops, 10s)   vs   A -> Fast -> Tail  (3 hops, 2ms)
        g.record_edge(a.clone(), slow.clone());
        g.record_edge(a.clone(), fast.clone());
        g.record_edge(fast.clone(), tail.clone());

        assert_eq!(g.critical_path(), vec![a, slow]);
    }

    #[test]
    fn critical_path_falls_back_to_hop_count_when_nothing_is_timed() {
        let (g, a, b, c, _d) = diamondless_chain();
        assert_eq!(g.critical_path(), vec![a, b, c]);
    }

    #[test]
    fn critical_path_of_an_empty_graph_is_empty() {
        assert_eq!(TraceGraph::new().critical_path(), Vec::<TraceRef>::new());
    }

    #[test]
    fn critical_path_terminates_on_a_cycle_by_ignoring_the_back_edge() {
        let (a, b) = (r(), r());
        let mut g = TraceGraph::new();
        g.record_edge(a.clone(), b.clone());
        g.record_edge(b.clone(), a.clone()); // not reachable through normal use

        let path = g.critical_path();
        assert_eq!(path.len(), 2);
        assert!(path.contains(&a) && path.contains(&b));
    }

    #[test]
    fn from_trace_sums_step_durations() {
        let mut t = Trace::new(1);
        t.push_step(Step::named("a").with_duration(Duration::from_millis(30)));
        t.push_step(Step::named("b").with_duration(Duration::from_millis(12)));
        t.push_step(Step::named("untimed"));

        let node = TraceNode::from_trace(&t, "Worker");
        assert_eq!(node.trace_ref, t.trace_ref());
        assert_eq!(node.duration, Some(Duration::from_millis(42)));
    }

    #[test]
    fn critical_path_keeps_the_first_recorded_chain_on_a_tie() {
        // Two identically-weighted chains: A -> B and A -> C. `best_of` must
        // keep the first-recorded successor, not the last one seen.
        let (a, b, c) = (r(), r(), r());
        let mut g = TraceGraph::new();
        g.record_node(TraceNode::new(a.clone(), "A"));
        g.record_node(TraceNode::new(b.clone(), "B"));
        g.record_node(TraceNode::new(c.clone(), "C"));
        g.record_edge(a.clone(), b.clone());
        g.record_edge(a.clone(), c.clone());

        assert_eq!(g.critical_path(), vec![a, b]);
    }

    #[test]
    fn graph_round_trips_through_json() {
        // `nodes` is keyed by `TraceRef`; JSON object keys must be strings,
        // so this is a real constraint on the representation, not a formality.
        let (g, a, _b, c, _d) = diamondless_chain();
        let json = serde_json::to_string(&g).expect("serializes");
        let restored: TraceGraph = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(restored.len(), g.len());
        assert_eq!(restored.edges(), g.edges());
        assert_eq!(restored.node(&a).unwrap().label, "A");
        assert_eq!(restored.downstream_of(&a).len(), 3);
        assert_eq!(restored.upstream_of(&c).len(), 2);
    }

    proptest::proptest! {
        /// `critical_path` must terminate and return a real path — every
        /// consecutive pair an actual edge, no node visited twice — for any
        /// edge set, including ones that are cyclic or disconnected.
        #[test]
        fn critical_path_is_always_a_valid_acyclic_walk(
            raw_edges in proptest::collection::vec((0usize..6, 0usize..6), 0..24)
        ) {
            let refs: Vec<TraceRef> = (0..6).map(|_| r()).collect();
            let mut g = TraceGraph::new();
            for (i, node) in refs.iter().enumerate() {
                g.record_node(TraceNode::new(node.clone(), format!("n{i}")));
            }
            for (from, to) in &raw_edges {
                g.record_edge(refs[*from].clone(), refs[*to].clone());
            }

            let path = g.critical_path();
            let unique: HashSet<&TraceRef> = path.iter().collect();
            proptest::prop_assert_eq!(unique.len(), path.len(), "path revisits a node");
            for pair in path.windows(2) {
                proptest::prop_assert!(
                    g.edges().contains(&(pair[0].clone(), pair[1].clone())),
                    "path traverses an edge that was never recorded"
                );
            }
            proptest::prop_assert!(!path.is_empty());
        }
    }
}
