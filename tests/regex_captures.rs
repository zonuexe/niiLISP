//! Regex capture variables `$0..$N` and regex-mode `find` / `replace`
//! (ADR-0028). `replace`'s regex form re-evaluates its expression per match.

#![cfg(feature = "regex")]

use std::process::Command;

fn eval(expr: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let out = Command::new(bin).arg("-e").arg(expr).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn regex_binds_capture_vars() {
    // $0 is the whole match, $1.. are the groups. (\\w reaches niiLISP as \w.)
    let out = eval(r#"(regex "(\\w+)@(\\w+)" "x bob@example y")(println $0 "|" $1 "|" $2)"#);
    assert_eq!(out, "bob@example|bob|example\n");
}

#[test]
fn find_regex_returns_offset_and_binds() {
    assert_eq!(eval(r#"(println (find "b+" "aabbb" 0))"#), "2\n");
    assert_eq!(eval(r#"(find "b+" "aabbb" 0)(println $0)"#), "bbb\n");
    // Literal find (no option) still works.
    assert_eq!(eval(r#"(println (find "an" "banana"))"#), "1\n");
}

#[test]
fn replace_regex_reevaluates_per_match() {
    // Uppercase each word via $0.
    assert_eq!(
        eval(r#"(set 'a "hello world")(println (replace "\\w+" a (upper-case $0) 0))"#),
        "HELLO WORLD\n"
    );
    // A side-effecting counter proves the expression runs once per match.
    assert_eq!(
        eval(r#"(set 'n 0)(set 'a "a a a")(println (replace "a" a (string (inc n)) 0) "|" n)"#),
        "1 2 3|3\n"
    );
    // Capture groups are usable in the replacement.
    assert_eq!(
        eval(r#"(set 'a "01-02")(println (replace "(\\d+)-(\\d+)" a (string $2 "/" $1) 0))"#),
        "02/01\n"
    );
}

#[test]
fn replace_literal_key_reevaluates_and_binds_dollar0() {
    // The 3-arg (no-option) form matches the key literally but still
    // re-evaluates the replacement per match, with $0 bound to the match.
    assert_eq!(
        eval(r#"(set 'n 0)(set 's "xaxbx")(replace "x" s (string (inc n)))(println s "|" n)"#),
        "1a2b3|3\n"
    );
    assert_eq!(
        eval(r#"(set 's "hello")(println (replace "l" s (upper-case $0)))"#),
        "heLLo\n"
    );
    // The literal key is regex-escaped: "." matches a real dot, not any char.
    assert_eq!(
        eval(r#"(set 's "a.b.c")(replace "." s "-")(println s)"#),
        "a-b-c\n"
    );
}

#[test]
fn replace_regex_is_destructive_and_no_match_is_identity() {
    assert_eq!(
        eval(r#"(set 'a "aXbXc")(replace "X" a "-" 0)(println a)"#),
        "a-b-c\n"
    );
    assert_eq!(eval(r#"(println (replace "z+" "abc" "!" 0))"#), "abc\n");
}
