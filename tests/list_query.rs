//! Higher-order list-query builtins from the WikiBook "Lists" chapter:
//! `clean`, `index`, `exists`, `for-all`, and `transpose`. Examples are taken
//! verbatim from the newLISP manual / WikiBook and compared to the real output.

use std::process::Command;

fn run(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-lq-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("p.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn clean_and_index_and_filter_agree() {
    // clean = filter with a negated predicate; index gives the true positions.
    let src = r#"
(println (clean symbol? '(1 2 d 4 f g 5 h)))
(println (index symbol? '(1 2 d 4 f g 5 h)))
(println (filter symbol? '(1 2 d 4 f g 5 h)))
(exit)
"#;
    let out = run("clean-index", src);
    assert!(out.contains("(1 2 4 5)"), "clean wrong:\n{}", out);
    assert!(out.contains("(2 4 5 7)"), "index wrong:\n{}", out);
    assert!(out.contains("(d f g h)"), "filter wrong:\n{}", out);
}

#[test]
fn exists_returns_first_match_or_nil() {
    let src = r#"
(println (exists string? '(2 3 4 6 "hello" 7)))
(println (exists string? '(2 3 4)))
(exit)
"#;
    let out = run("exists", src);
    assert!(out.contains("hello"), "exists match wrong:\n{}", out);
    assert!(out.contains("nil"), "exists miss wrong:\n{}", out);
}

#[test]
fn for_all_checks_every_element() {
    let src = r#"
(println (for-all number? '(2 3 4 6 7)))
(println (for-all number? '(2 3 4 6 "hello" 7)))
(exit)
"#;
    let out = run("for-all", src);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some("true"),
        "for-all all:\n{}",
        out
    );
    assert_eq!(lines.get(1).copied(), Some("nil"), "for-all some:\n{}", out);
}

#[test]
fn transpose_swaps_rows_and_columns() {
    // Rectangular case, plus a ragged matrix padded with nil.
    let src = r#"
(println (transpose '((1 2 3) (4 5 6))))
(println (transpose '((1 2 3) (4 5))))
(exit)
"#;
    let out = run("transpose", src);
    assert!(
        out.contains("((1 4) (2 5) (3 6))"),
        "transpose rect wrong:\n{}",
        out
    );
    assert!(
        out.contains("((1 4) (2 5) (3 nil))"),
        "transpose ragged wrong:\n{}",
        out
    );
}
