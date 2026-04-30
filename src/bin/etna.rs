// ETNA workload runner for quick-xml.
//
// Usage: cargo run --release --bin etna -- <tool> <property>
//   tool:     etna | proptest | quickcheck | crabcheck | hegel
//   property: UnescapePredefEntities | EscapeCharCodes | All
//
// Each run emits a single JSON line on stdout with fields:
//   status, tests, discards, time, counterexample, error, tool, property.
// Exit status is always 0 on completion; non-zero exit is reserved for
// adapter-level panics that escape the catch_unwind in main().

use crabcheck::quickcheck as crabcheck_qc;
use hegel::{generators as hgen, Hegel, Settings as HegelSettings};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestRunner};
use quick_xml::etna::{
    property_escape_char_codes, property_unescape_predef_entities, PropertyResult,
};
use quickcheck::{Arbitrary, Gen, QuickCheck, ResultStatus, TestResult};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default, Clone, Copy)]
struct Metrics {
    inputs: u64,
    elapsed_us: u128,
}

impl Metrics {
    fn combine(self, other: Metrics) -> Metrics {
        Metrics {
            inputs: self.inputs + other.inputs,
            elapsed_us: self.elapsed_us + other.elapsed_us,
        }
    }
}

type Outcome = (Result<(), String>, Metrics);

fn to_err(r: PropertyResult) -> Result<(), String> {
    match r {
        PropertyResult::Pass | PropertyResult::Discard => Ok(()),
        PropertyResult::Fail(m) => Err(m),
    }
}

const ALL_PROPERTIES: &[&str] = &["UnescapePredefEntities", "EscapeCharCodes"];

fn run_all<F: FnMut(&str) -> Outcome>(mut f: F) -> Outcome {
    let mut total = Metrics::default();
    let mut final_status: Result<(), String> = Ok(());
    for p in ALL_PROPERTIES {
        let (r, m) = f(p);
        total = total.combine(m);
        if r.is_err() && final_status.is_ok() {
            final_status = r;
        }
    }
    (final_status, total)
}

// ---- etna (deterministic witness-shaped inputs) ----

fn run_etna_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_etna_property);
    }
    let t0 = Instant::now();
    let result = match property {
        "UnescapePredefEntities" => to_err(property_unescape_predef_entities(b"&lt;".to_vec())),
        "EscapeCharCodes" => to_err(property_escape_char_codes(b'\n')),
        _ => {
            return (
                Err(format!("Unknown property for etna: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    (
        result,
        Metrics {
            inputs: 1,
            elapsed_us,
        },
    )
}

// ---- proptest ----

fn predef_entities_strategy() -> BoxedStrategy<Vec<u8>> {
    // Bias the strategy toward inputs containing known XML predefined entity
    // references — purely-random bytes get rejected by the property's domain
    // filter and yield only Discards.
    let entity_token = prop_oneof![
        Just(b"&lt;".to_vec()),
        Just(b"&gt;".to_vec()),
        Just(b"&amp;".to_vec()),
        Just(b"&apos;".to_vec()),
        Just(b"&quot;".to_vec()),
        prop::collection::vec(prop::char::range('a', 'z').prop_map(|c| c as u8), 1..4),
    ];
    prop::collection::vec(entity_token, 1..6)
        .prop_map(|chunks| chunks.into_iter().flatten().collect())
        .boxed()
}

fn escape_char_strategy() -> BoxedStrategy<u8> {
    // Bias toward bytes the property accepts: <, >, ', &, ", \t, \n, \r, ' '.
    prop_oneof![
        Just(b'<'),
        Just(b'>'),
        Just(b'\''),
        Just(b'&'),
        Just(b'"'),
        Just(b'\t'),
        Just(b'\n'),
        Just(b'\r'),
        Just(b' '),
        any::<u8>(),
    ]
    .boxed()
}

fn run_proptest_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_proptest_property);
    }
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let mut runner = TestRunner::new(ProptestConfig::default());
    let c = counter.clone();
    let result: Result<(), String> = match property {
        "UnescapePredefEntities" => runner
            .run(&predef_entities_strategy(), move |args| {
                c.fetch_add(1, Ordering::Relaxed);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_unescape_predef_entities(args.clone())
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => Err(TestCaseError::fail(format!(
                        "({:?})",
                        String::from_utf8_lossy(&args)
                    ))),
                }
            })
            .map_err(|e| match e {
                proptest::test_runner::TestError::Fail(r, _) => r.to_string(),
                other => other.to_string(),
            }),
        "EscapeCharCodes" => runner
            .run(&escape_char_strategy(), move |args| {
                c.fetch_add(1, Ordering::Relaxed);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_escape_char_codes(args)
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        Err(TestCaseError::fail(format!("({:?})", args)))
                    }
                }
            })
            .map_err(|e| match e {
                proptest::test_runner::TestError::Fail(r, _) => r.to_string(),
                other => other.to_string(),
            }),
        _ => {
            return (
                Err(format!("Unknown property for proptest: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = counter.load(Ordering::Relaxed);
    (
        result,
        Metrics {
            inputs,
            elapsed_us,
        },
    )
}

// ---- quickcheck ----

static QC_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Newtype wrapper used purely so the quickcheck fork has a `Display`-able
/// argument type and a size-0-safe `Arbitrary` impl. The fork's `quicktest`
/// loop sets `gen.size()` to `log2(i) as usize` and so passes `size = 0` on
/// the first iteration, which makes `Vec::arbitrary`'s `random_range(0..size)`
/// panic with "cannot sample empty range". This wrapper picks from a fixed
/// token table so it is independent of `gen.size()`.
#[derive(Clone, Debug)]
struct EntityTokenString(Vec<u8>);

impl std::fmt::Display for EntityTokenString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl Arbitrary for EntityTokenString {
    fn arbitrary(g: &mut Gen) -> Self {
        let token_table: &[&[u8]] = &[
            b"&lt;", b"&gt;", b"&amp;", b"&apos;", b"&quot;", b"a", b"b", b"c", b"x", b"1",
        ];
        let count = *g.choose(&[1usize, 2, 3, 4, 5]).unwrap_or(&3);
        let mut buf = Vec::new();
        for _ in 0..count {
            let pick = g.choose(token_table).unwrap_or(&token_table[0]);
            buf.extend_from_slice(pick);
        }
        EntityTokenString(buf)
    }
}

fn qc_unescape_predef_entities(EntityTokenString(bytes): EntityTokenString) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_unescape_predef_entities(bytes) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn qc_escape_char_codes(b: u8) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_escape_char_codes(b) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn run_quickcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_quickcheck_property);
    }
    QC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let result = match property {
        "UnescapePredefEntities" => QuickCheck::new()
            .tests(200)
            .max_tests(20_000)
            .max_time(Duration::from_secs(86_400))
            .quicktest(qc_unescape_predef_entities as fn(EntityTokenString) -> TestResult),
        "EscapeCharCodes" => QuickCheck::new()
            .tests(200)
            .max_tests(20_000)
            .max_time(Duration::from_secs(86_400))
            .quicktest(qc_escape_char_codes as fn(u8) -> TestResult),
        _ => {
            return (
                Err(format!("Unknown property for quickcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = QC_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics {
        inputs,
        elapsed_us,
    };
    let status = match result.status {
        ResultStatus::Finished => Ok(()),
        ResultStatus::Failed { arguments } => Err(format!("({})", arguments.join(" "))),
        ResultStatus::Aborted { err } => Err(format!("aborted: {err:?}")),
        ResultStatus::TimedOut => Err("timed out".to_string()),
        ResultStatus::GaveUp => Err(format!(
            "gave up: passed={}, discarded={}",
            result.n_tests_passed, result.n_tests_discarded
        )),
    };
    (status, metrics)
}

// ---- crabcheck ----

static CC_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Token-biased newtype around `Vec<u8>` for crabcheck. Crabcheck's default
/// `Vec<u8>` generator picks each byte uniformly, so the chance of stumbling
/// on `&lt;` (4 specific bytes in a row) is roughly `1 / 2^32` — far below
/// the 20k-trial budget. This wrapper picks from a fixed token table instead.
#[derive(Clone)]
struct CcEntityTokenString(Vec<u8>);

impl std::fmt::Debug for CcEntityTokenString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl std::fmt::Display for CcEntityTokenString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl<R: rand::Rng> crabcheck::quickcheck::Arbitrary<R> for CcEntityTokenString {
    fn generate(rng: &mut R, _n: usize) -> Self {
        let token_table: &[&[u8]] = &[
            b"&lt;", b"&gt;", b"&amp;", b"&apos;", b"&quot;", b"a", b"b", b"c", b"x", b"1",
        ];
        let count = rng.random_range(1..=5usize);
        let mut buf = Vec::new();
        for _ in 0..count {
            let idx = rng.random_range(0..token_table.len());
            buf.extend_from_slice(token_table[idx]);
        }
        CcEntityTokenString(buf)
    }
}

fn cc_unescape_predef_entities(CcEntityTokenString(bytes): CcEntityTokenString) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_unescape_predef_entities(bytes) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_escape_char_codes(b: u8) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_escape_char_codes(b) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn run_crabcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_crabcheck_property);
    }
    CC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let cfg = crabcheck_qc::Config { tests: 20_000 };
    let result = match property {
        "UnescapePredefEntities" => crabcheck_qc::quickcheck_with_config(
            cfg,
            cc_unescape_predef_entities as fn(CcEntityTokenString) -> Option<bool>,
        ),
        "EscapeCharCodes" => crabcheck_qc::quickcheck_with_config(
            cfg,
            cc_escape_char_codes as fn(u8) -> Option<bool>,
        ),
        _ => {
            return (
                Err(format!("Unknown property for crabcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = CC_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics {
        inputs,
        elapsed_us,
    };
    let status = match result.status {
        crabcheck_qc::ResultStatus::Finished => Ok(()),
        crabcheck_qc::ResultStatus::Failed { arguments } => {
            Err(format!("({})", arguments.join(" ")))
        }
        crabcheck_qc::ResultStatus::TimedOut => Err("timed out".to_string()),
        crabcheck_qc::ResultStatus::GaveUp => Err(format!(
            "gave up: passed={}, discarded={}",
            result.passed, result.discarded
        )),
        crabcheck_qc::ResultStatus::Aborted { error } => Err(format!("aborted: {error}")),
    };
    (status, metrics)
}

// ---- hegel ----

static HG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn hegel_settings() -> HegelSettings {
    HegelSettings::new()
        .test_cases(200)
        .suppress_health_check(hegel::HealthCheck::all())
}

fn run_hegel_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_hegel_property);
    }
    HG_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let settings = hegel_settings();
    let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match property {
        "UnescapePredefEntities" => {
            Hegel::new(|tc: hegel::TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                // Build a Vec<u8> by concatenating predefined entity tokens
                // and ASCII letters. Hegel's bytes generator alone is too
                // unstructured — it discards almost every input.
                let token_count = (tc.draw(hgen::integers::<u8>()) % 6 + 1) as usize;
                let mut buf = Vec::new();
                for _ in 0..token_count {
                    let pick = tc.draw(hgen::integers::<u8>()) % 6;
                    match pick {
                        0 => buf.extend_from_slice(b"&lt;"),
                        1 => buf.extend_from_slice(b"&gt;"),
                        2 => buf.extend_from_slice(b"&amp;"),
                        3 => buf.extend_from_slice(b"&apos;"),
                        4 => buf.extend_from_slice(b"&quot;"),
                        _ => {
                            let len = (tc.draw(hgen::integers::<u8>()) % 3 + 1) as usize;
                            for _ in 0..len {
                                let c =
                                    b'a' + (tc.draw(hgen::integers::<u8>()) % 26);
                                buf.push(c);
                            }
                        }
                    }
                }
                let cex = format!("({:?})", String::from_utf8_lossy(&buf));
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_unescape_predef_entities(buf.clone())
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{cex}"),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "EscapeCharCodes" => {
            Hegel::new(|tc: hegel::TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                // Bias toward the 9 bytes the property accepts.
                let pool: &[u8] = &[
                    b'<', b'>', b'\'', b'&', b'"', b'\t', b'\n', b'\r', b' ',
                ];
                let pick = tc.draw(hgen::integers::<u8>());
                let b = if (pick % 4) == 0 {
                    tc.draw(hgen::integers::<u8>())
                } else {
                    pool[(pick as usize) % pool.len()]
                };
                let cex = format!("({:?})", b as char);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_escape_char_codes(b)
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{cex}"),
                }
            })
            .settings(settings.clone())
            .run();
        }
        _ => panic!("__unknown_property:{property}"),
    }));
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = HG_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics {
        inputs,
        elapsed_us,
    };
    let status = match run_result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "hegel panicked with non-string payload".to_string()
            };
            if let Some(rest) = msg.strip_prefix("__unknown_property:") {
                return (
                    Err(format!("Unknown property for hegel: {rest}")),
                    Metrics::default(),
                );
            }
            Err(msg
                .strip_prefix("Property test failed: ")
                .unwrap_or(&msg)
                .to_string())
        }
    };
    (status, metrics)
}

fn run(tool: &str, property: &str) -> Outcome {
    match tool {
        "etna" => run_etna_property(property),
        "proptest" => run_proptest_property(property),
        "quickcheck" => run_quickcheck_property(property),
        "crabcheck" => run_crabcheck_property(property),
        "hegel" => run_hegel_property(property),
        _ => (Err(format!("Unknown tool: {tool}")), Metrics::default()),
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn emit_json(
    tool: &str,
    property: &str,
    status: &str,
    metrics: Metrics,
    counterexample: Option<&str>,
    error: Option<&str>,
) {
    let cex = counterexample.map_or("null".to_string(), json_str);
    let err = error.map_or("null".to_string(), json_str);
    println!(
        "{{\"status\":{},\"tests\":{},\"discards\":0,\"time\":{},\"counterexample\":{},\"error\":{},\"tool\":{},\"property\":{}}}",
        json_str(status),
        metrics.inputs,
        json_str(&format!("{}us", metrics.elapsed_us)),
        cex,
        err,
        json_str(tool),
        json_str(property),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <tool> <property>", args[0]);
        eprintln!("Tools: etna | proptest | quickcheck | crabcheck | hegel");
        eprintln!("Properties: UnescapePredefEntities | EscapeCharCodes | All");
        std::process::exit(2);
    }
    let (tool, property) = (args[1].as_str(), args[2].as_str());

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(tool, property)));
    std::panic::set_hook(previous_hook);

    let (result, metrics) = match caught {
        Ok(outcome) => outcome,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "panic with non-string payload".to_string()
            };
            emit_json(
                tool,
                property,
                "aborted",
                Metrics::default(),
                None,
                Some(&format!("adapter panic: {msg}")),
            );
            return;
        }
    };

    match result {
        Ok(()) => emit_json(tool, property, "passed", metrics, None, None),
        Err(msg) => emit_json(tool, property, "failed", metrics, Some(&msg), None),
    }
}
