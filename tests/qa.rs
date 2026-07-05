//! Integration tests that run vendored newLISP qa scripts through the built
//! interpreter and check their self-reported result (ADR-0009).
//!
//! These scripts call `(exit)`, so they must run as a subprocess rather than
//! in-process.

use std::process::Command;

fn run_qa(name: &str) -> String {
    run_qa_args(name, &[])
}

/// Run a vendored qa script with extra command-line arguments (available to the
/// script via `(main-args)`), returning its stdout.
fn run_qa_args(name: &str, extra: &[&str]) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let path = format!("references/newlisp/qa-specific-tests/{}", name);
    let output = Command::new(bin)
        .arg(&path)
        .args(extra)
        .output()
        .unwrap_or_else(|e| panic!("failed to launch {}: {}", bin, e));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn qa_exception_passes() {
    let stdout = run_qa("qa-exception");
    assert!(
        stdout.contains("SUCCESSFUL"),
        "qa-exception did not report success:\n{}",
        stdout
    );
}

/// The FFI memory-API slice (ADR-0021): NULL `char*` through `unpack` and
/// `get-string` must error rather than segfault. Needs `struct`/`pack`/`unpack`/
/// `get-string`, which are Unix-only and gated on the `ffi` feature.
#[test]
#[cfg(all(feature = "ffi", unix))]
fn qa_nullstring_passes() {
    let stdout = run_qa("qa-nullstring");
    assert!(
        stdout.contains("SUCCESS unpacking NULL ptr"),
        "qa-nullstring unpack did not report success:\n{}",
        stdout
    );
    assert!(
        stdout.contains("SUCCESS get-string on NULL ptr"),
        "qa-nullstring get-string did not report success:\n{}",
        stdout
    );
}

/// bigint (ADR-0022) plus the helper functions it uses (`seed`/`rand`/`random`/
/// `amb`/`until`/`extend`/`explode`/`main-args`). The script fuzzes the bigint
/// operators over `N` random cases; `N` defaults to 100000 but reads a smaller
/// value from `(main-args -1)`, so a passed argument keeps the test quick while
/// still exercising every operator.
#[cfg(feature = "bigint")]
#[test]
fn qa_bigint_passes() {
    let stdout = run_qa_args("qa-bigint", &["300"]);
    assert!(
        stdout.contains("big ints tested SUCCESSFUL"),
        "qa-bigint did not report success:\n{}",
        stdout
    );
}

/// Factor the first N Fibonacci numbers, using an array-based sieve (ADR-0023)
/// and bigint. Passes, but its `collect-primes` sieves to 1,000,000, which is
/// ~1s release / ~10s debug even at N=12 — too slow for the default run, so it
/// is `#[ignore]`d (run with `cargo test -- --ignored`). Copy-on-write
/// (ADR-0024) is what makes the sieve feasible at all.
#[cfg(feature = "bigint")]
#[test]
#[ignore = "1,000,000-element sieve is ~10s in debug; run explicitly"]
fn qa_factorfibo_passes() {
    let stdout = run_qa_args("qa-factorfibo", &["12"]);
    assert!(
        stdout.contains("Total duration:") && !stdout.contains("ERROR"),
        "qa-factorfibo did not finish cleanly:\n{}",
        stdout
    );
    // Spot-check a known factorization: fibo 11 = 144 = 2^4 * 3^2.
    assert!(
        stdout.contains("144 -> (2 2 2 2 3 3)"),
        "qa-factorfibo factorization looks wrong:\n{}",
        stdout
    );
}

/// A 1000-digit literal squared and divided back, with a per-digit checksum —
/// bigint parsing/printing plus `explode`/`chop`.
#[cfg(feature = "bigint")]
#[test]
fn qa_longnum_passes() {
    let stdout = run_qa("qa-longnum");
    assert!(
        stdout.contains("parsing big integers SUCCESSFUL"),
        "qa-longnum did not report success:\n{}",
        stdout
    );
}

#[test]
fn qa_foop_passes() {
    let stdout = run_qa("qa-foop");
    assert!(
        stdout.contains("FOOP nested 'self' tested SUCCCESSFUL"),
        "qa-foop nested-self did not pass:\n{}",
        stdout
    );
    assert!(
        stdout.contains("FOOP symbol protection SUCCESSFUL"),
        "qa-foop symbol protection did not pass:\n{}",
        stdout
    );
}
