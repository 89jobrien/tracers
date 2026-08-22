#![no_main]
//! Build a `Trace<String>` from arbitrary input and prove it survives a
//! serialization round trip byte-for-byte.
//!
//! CLAUDE.md states the workspace invariant as "all types are
//! `Serialize + Deserialize` — a compile-time constraint, not a convention".
//! The compiler proves the impls exist. It does not prove they agree with
//! each other, which is what a checkpoint written today and read back next
//! week actually depends on.

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use trace_lang_core::{Branch, Step, StepCost, Trace};

/// A recipe for a trace. Fuzzing the *builder inputs* rather than raw JSON
/// keeps every generated case a trace the library itself could have produced.
#[derive(Debug, Arbitrary)]
struct TraceRecipe {
    value: String,
    steps: Vec<StepRecipe>,
}

#[derive(Debug, Arbitrary)]
struct StepRecipe {
    name: String,
    confidence: Option<f64>,
    duration_micros: Option<u64>,
    note: Option<String>,
    cost: Option<(u64, u64, Option<f64>)>,
    branches: Vec<(String, bool, Option<f64>)>,
}

fn build(recipe: TraceRecipe) -> Trace<String> {
    let mut trace = Trace::new(recipe.value);
    for spec in recipe.steps {
        let mut step = Step::named(spec.name);
        if let Some(confidence) = spec.confidence {
            // NaN would make the round trip meaningless rather than wrong:
            // JSON has no NaN, so serde rejects it before anything else can.
            if confidence.is_finite() {
                step = step.with_confidence(confidence);
            }
        }
        if let Some(micros) = spec.duration_micros {
            step = step.with_duration(std::time::Duration::from_micros(micros));
        }
        if let Some(note) = spec.note {
            step = step.with_note(note);
        }
        if let Some((input, output, dollars)) = spec.cost {
            let mut cost = StepCost::new(input, output);
            if let Some(dollars) = dollars.filter(|d| d.is_finite()) {
                cost = cost.with_dollars(dollars);
            }
            step = step.with_cost(cost);
        }
        for (label, taken, confidence) in spec.branches {
            let mut branch = if taken {
                Branch::taken(label)
            } else {
                Branch::rejected(label, "fuzzed")
            };
            if let Some(confidence) = confidence.filter(|c| c.is_finite()) {
                branch = branch.with_confidence(confidence);
            }
            step.branches.push(branch);
        }
        trace.push_step(step);
    }
    trace
}

fuzz_target!(|recipe: TraceRecipe| {
    let trace = build(recipe);

    let first = match serde_json::to_string(&trace) {
        Ok(json) => json,
        // A value the serializer legitimately refuses is not a bug; a panic
        // would be.
        Err(_) => return,
    };

    let once: Trace<String> =
        serde_json::from_str(&first).expect("a trace this crate serialized must deserialize");
    let second = serde_json::to_string(&once).expect("and must re-serialize");
    let twice: Trace<String> = serde_json::from_str(&second).expect("and deserialize again");
    let third = serde_json::to_string(&twice).expect("and re-serialize again");

    // This assertion is why the workspace enables serde_json's
    // `float_roundtrip` feature. Without it, this fuzzer falsifies the
    // invariant within seconds: a `confidence` of 1.5626343493868385e-307
    // came back as ...383e-307, one ULP off, and kept drifting on subsequent
    // cycles. serde_json's default float parser is not exact for every
    // extreme-exponent f64, and `with_confidence` clamps to [0.0, 1.0]
    // without excluding absurdly small magnitudes, so such a value is
    // constructible — and a checkpoint that changes a little on every
    // save/load cycle is not a checkpoint.
    assert_eq!(
        first, second,
        "a trace must serialize identically before and after a round trip"
    );
    assert_eq!(second, third, "and stay that way on every subsequent cycle");

    // Everything that is not a float must survive the very first trip exactly.
    assert_eq!(trace.id, once.id, "a trace must keep its identity");
    assert_eq!(
        trace.causal_chain().len(),
        once.causal_chain().len(),
        "the causal chain must survive the round trip intact"
    );
    for (before, after) in trace.causal_chain().iter().zip(once.causal_chain()) {
        assert_eq!(before.id, after.id);
        assert_eq!(before.name, after.name);
        assert_eq!(before.outcome, after.outcome);
        assert_eq!(before.notes, after.notes);
        assert_eq!(before.duration, after.duration);
        assert_eq!(before.started_at, after.started_at);
        assert_eq!(
            before.cost.map(|c| (c.input_tokens, c.output_tokens)),
            after.cost.map(|c| (c.input_tokens, c.output_tokens)),
        );
        assert_eq!(before.branches.len(), after.branches.len());
    }
});
