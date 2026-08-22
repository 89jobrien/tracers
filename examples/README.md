# trace:: examples

Runnable, printable walkthroughs of the four crates. Each one is standalone —
read it top to bottom, or run it and read the output.

```bash
cargo run -p trace-lang-examples --example trace_basics
```

| example             | what it shows                                                                              |
| ------------------- | ------------------------------------------------------------------------------------------ |
| `trace_basics`      | building a `Trace<T>` and querying it — causal chain, bottlenecks, cost, doubt, branches     |
| `trace_graph`       | `TraceGraph` lineage across runs: downstream, upstream, and the critical path                |
| `task_pipeline`     | `TaskRegistry` dependency gating, and resuming from a checkpoint after a crash               |
| `agent_escalation`  | `spawn`, hook-driven delegation resolved through `AgentRegistry`, `join_all`, `speculate`    |
| `contracts`         | pre/post-conditions catching output that succeeded and is still wrong                        |
| `human_in_the_loop` | `RequireApproval` → `TaskStatus::Paused` → resume, across a process exit                     |

`cargo test --workspace` compiles all of them, so an API change that breaks an
example breaks the build rather than rotting quietly.

Shared scaffolding (output formatting, scratch paths, the `Drafter`/`Editor`
pair) lives in `src/lib.rs`. The crate is `publish = false`.
