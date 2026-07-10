//! `curry` (partial application) and `global`/`global?` (cross-context symbol
//! visibility) from the WikiBook. Examples follow the newLISP manual.

use std::process::Command;

fn run(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-cg-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("p.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn curry_makes_a_one_arg_function() {
    let src = r#"
(set 'f (curry + 10))
(println (f 7))
(println (curry + 10))
(println (map (curry + 100) '(1 2 3)))
(exit)
"#;
    let out = run("curry", src);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.first().copied(), Some("17"), "curry apply:\n{}", out);
    assert_eq!(
        lines.get(1).copied(),
        Some("(lambda ($x) (+ 10 $x))"),
        "curry shape:\n{}",
        out
    );
    assert_eq!(
        lines.get(2).copied(),
        Some("(101 102 103)"),
        "curry in map:\n{}",
        out
    );
}

#[test]
fn curry_does_not_evaluate_its_arguments() {
    // The exp is spliced literally and evaluated only on application, so a
    // quoted pattern survives intact.
    let src = r#"
(println (curry match '(a *)))
(exit)
"#;
    let out = run("curry-noeval", src);
    assert!(
        out.contains("(lambda ($x) (match (quote (a *)) $x))")
            || out.contains("(lambda ($x) (match '(a *) $x))"),
        "curry should not evaluate exp:\n{}",
        out
    );
}

#[test]
fn global_returns_last_and_global_p_reports() {
    let src = r#"
(println (global 'aVar 'x 'y 'z))
(println (global? 'print))
(global 'myvar)
(println (global? 'myvar))
(println (global? 'neverdeclared))
(exit)
"#;
    let out = run("global", src);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some("z"),
        "global returns last:\n{}",
        out
    );
    assert_eq!(
        lines.get(1).copied(),
        Some("true"),
        "global? builtin:\n{}",
        out
    );
    assert_eq!(
        lines.get(2).copied(),
        Some("true"),
        "global? declared:\n{}",
        out
    );
    assert_eq!(
        lines.get(3).copied(),
        Some("nil"),
        "global? undeclared:\n{}",
        out
    );
}

#[test]
fn global_enables_the_constant_alias_pattern() {
    // The WikiBook "On your own terms" idiom: (constant (global 'name) fn).
    let src = r#"
(constant (global 'set!) set)
(set! 'q 5)
(println q)
(exit)
"#;
    let out = run("global-alias", src);
    assert!(out.contains('5'), "constant+global alias:\n{}", out);
}
