//! `series` (geometric and functional sequences) and `factor` (prime
//! factorization), from the WikiBook "Working with numbers" chapter. Examples
//! follow the newLISP manual.

use std::process::Command;

fn run(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-sf-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("p.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn series_geometric() {
    let src = r#"
(println (series 2 2 5))
(println (series 1 1.2 6))
(println (series 10 0.9 4))
(println (series 5 2 0))
(exit)
"#;
    let out = run("series-geo", src);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some("(2 4 8 16 32)"),
        "geo:\n{}",
        out
    );
    assert_eq!(
        lines.get(1).copied(),
        Some("(1 1.2 1.44 1.728 2.0736 2.48832)"),
        "geo float:\n{}",
        out
    );
    assert_eq!(
        lines.get(2).copied(),
        Some("(10 9 8.1 7.29)"),
        "geo decay:\n{}",
        out
    );
    assert_eq!(lines.get(3).copied(), Some("()"), "count<1 empty:\n{}", out);
}

#[test]
fn series_functional() {
    // Each term is (func previous).
    let src = r#"
(println (series 100 (fn (x) (/ x 2)) 4))
(exit)
"#;
    let out = run("series-fn", src);
    assert!(
        out.contains("(100 50 25 12)"),
        "functional series:\n{}",
        out
    );
}

#[test]
fn factor_prime_decomposition() {
    let src = r#"
(println (factor 12))
(println (factor 123456789123456789))
(println (= (apply * (factor 123456789123456789)) 123456789123456789))
(println (factor 9223372036854775807))
(println (factor 7))
(println (factor 1))
(exit)
"#;
    let out = run("factor", src);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some("(2 2 3)"),
        "factor 12:\n{}",
        out
    );
    assert_eq!(
        lines.get(1).copied(),
        Some("(3 3 7 11 13 19 3607 3803 52579)"),
        "factor big:\n{}",
        out
    );
    assert_eq!(
        lines.get(2).copied(),
        Some("true"),
        "product check:\n{}",
        out
    );
    assert_eq!(
        lines.get(3).copied(),
        Some("(7 7 73 127 337 92737 649657)"),
        "factor i64 max:\n{}",
        out
    );
    assert_eq!(lines.get(4).copied(), Some("(7)"), "factor prime:\n{}", out);
    assert_eq!(
        lines.get(5).copied(),
        Some("nil"),
        "factor 1 → nil:\n{}",
        out
    );
}
