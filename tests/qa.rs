//! Integration tests that run vendored newLISP qa scripts through the built
//! interpreter and check their self-reported result (ADR-0009).
//!
//! These scripts call `(exit)`, so they must run as a subprocess rather than
//! in-process.

use std::process::Command;

fn run_qa(name: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let path = format!("references/newlisp/qa-specific-tests/{}", name);
    let output = Command::new(bin)
        .arg(&path)
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
