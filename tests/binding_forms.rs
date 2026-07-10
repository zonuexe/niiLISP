//! The `letn` (sequential) and `letex` (let + expand) binding special forms,
//! plus the `let` syntax improvements they share: the fully-parenthesized
//! binding form and a bare symbol defaulting to `nil`. Examples follow the
//! newLISP manual / WikiBook.

use std::process::Command;

fn run(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-bind-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("p.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn letn_binds_sequentially() {
    // Each initializer sees the bindings made before it.
    let src = r#"
(println (letn (x 2 y (pow x 3) z (pow x 4)) (list x y z)))
(println (letn ((a 1) (b (+ a 1)) (c)) (list a b c)))
(exit)
"#;
    let out = run("letn", src);
    assert!(out.contains("(2 8 16)"), "letn flat wrong:\n{}", out);
    assert!(
        out.contains("(1 2 nil)"),
        "letn paren/optional wrong:\n{}",
        out
    );
}

#[test]
fn letex_expands_values_into_body() {
    let src = r#"
(println (letex (x 1 y 2 z 3) '(x y z)))
(println (letex ((x 1) (y '(a b c)) (z "hello")) '(x y z)))
(exit)
"#;
    let out = run("letex", src);
    assert!(out.contains("(1 2 3)"), "letex flat wrong:\n{}", out);
    assert!(
        out.contains("(1 (a b c) \"hello\")"),
        "letex paren wrong:\n{}",
        out
    );
}

#[test]
fn let_is_still_parallel() {
    // The new local `x` must NOT be visible to a sibling initializer.
    let src = r#"
(set 'x 100)
(println (let (x 1 y x) y))
(exit)
"#;
    let out = run("let-parallel", src);
    assert!(out.contains("100"), "let should be parallel:\n{}", out);
}

#[test]
fn let_accepts_paren_form_and_bare_symbol() {
    let src = r#"
(println (let ((a 3) (b 4)) (+ a b)))
(println (let (y) y))
(exit)
"#;
    let out = run("let-syntax", src);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some("7"),
        "let paren form:\n{}",
        out
    );
    assert_eq!(
        lines.get(1).copied(),
        Some("nil"),
        "let bare symbol → nil:\n{}",
        out
    );
}
