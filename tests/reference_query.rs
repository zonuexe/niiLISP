//! The reference/query model (ADR-0036): `ref`, `ref-all`, `match`, `find-all`,
//! `pop-assoc`, and the `push`/`pop` index-vector forms. Examples follow the
//! newLISP manual / WikiBook.

use std::process::Command;

fn run(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-rq-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("p.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn ref_returns_index_path() {
    let src = r#"
(println (ref 4 '((1 2) (1 2 3) (1 2 3 4))))
(println (ref 'c '(a (b c))))
(println (ref 'z '(a b c)))
(exit)
"#;
    let out = run("ref", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(l.first().copied(), Some("(2 3)"), "nested ref:\n{}", out);
    assert_eq!(l.get(1).copied(), Some("(1 1)"), "ref path:\n{}", out);
    assert_eq!(l.get(2).copied(), Some("()"), "ref not found:\n{}", out);
}

#[test]
fn ref_all_paths_elements_and_count() {
    let src = r#"
(println (ref-all 'a '(a (b a) c a)))
(println (ref-all 'a '(a (b a) c a) = true))
(ref-all 'a '(a (b a) c a))
(println $count)
(exit)
"#;
    let out = run("ref-all", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(
        l.first().copied(),
        Some("((0) (1 1) (3))"),
        "paths:\n{}",
        out
    );
    assert_eq!(l.get(1).copied(), Some("(a a a)"), "elements:\n{}", out);
    assert_eq!(l.get(2).copied(), Some("3"), "$count:\n{}", out);
}

#[test]
fn match_wildcards_and_nesting() {
    let src = r#"
(println (match '(1 ? 3) '(1 2 3)))
(println (match '(1 * 5) '(1 2 3 4 5)))
(println (match '(* 3 *) '(1 2 3 4)))
(println (match '(a (? c) *) '(a (b c) d e)))
(println (match '(1 2) '(1 3)))
(exit)
"#;
    let out = run("match", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(l.first().copied(), Some("(2)"), "? wildcard:\n{}", out);
    assert_eq!(l.get(1).copied(), Some("((2 3 4))"), "* middle:\n{}", out);
    assert_eq!(
        l.get(2).copied(),
        Some("((1 2) (4))"),
        "backtrack:\n{}",
        out
    );
    assert_eq!(l.get(3).copied(), Some("(b (d e))"), "nested:\n{}", out);
    assert_eq!(l.get(4).copied(), Some("nil"), "no match:\n{}", out);
}

#[test]
fn push_pop_index_vectors() {
    let src = r#"
(set 'L '(a (b c) d))
(println (pop L (ref 'c L)))
(println L)
(set 'M '((1 2) (3 4)))
(push 'X M 1 1)
(println M)
(println (pop M '(0 0)))
(println M)
(exit)
"#;
    let out = run("push-pop", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(l.first().copied(), Some("c"), "pop via ref:\n{}", out);
    assert_eq!(
        l.get(1).copied(),
        Some("(a (b) d)"),
        "L after pop:\n{}",
        out
    );
    assert_eq!(
        l.get(2).copied(),
        Some("((1 2) (3 X 4))"),
        "push path:\n{}",
        out
    );
    assert_eq!(l.get(3).copied(), Some("1"), "pop path:\n{}", out);
    assert_eq!(
        l.get(4).copied(),
        Some("((2) (3 X 4))"),
        "M after:\n{}",
        out
    );
}

#[test]
fn find_all_forms() {
    let src = r#"
(println (find-all "wo.d" "word work wold ward"))
(println (find-all "\\w+" "the quick fox" (upper-case $0)))
(println (find-all '(a ?) '((a 1) (b 2) (a 3) (c 4))))
(println (find-all 3 '(1 3 2 3 3)))
(find-all "o" "foo boo")
(println $count)
(exit)
"#;
    let out = run("find-all", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(
        l.first().copied(),
        Some("(\"word\" \"wold\")"),
        "regex:\n{}",
        out
    );
    assert_eq!(
        l.get(1).copied(),
        Some("(\"THE\" \"QUICK\" \"FOX\")"),
        "regex transform:\n{}",
        out
    );
    assert_eq!(
        l.get(2).copied(),
        Some("((a 1) (a 3))"),
        "list pattern:\n{}",
        out
    );
    assert_eq!(l.get(3).copied(), Some("(3 3 3)"), "key:\n{}", out);
    assert_eq!(l.get(4).copied(), Some("4"), "$count:\n{}", out);
}

#[test]
fn pop_assoc_removes_and_returns_pair() {
    let src = r#"
(set 'L '((a 1) (b 2) (c 3)))
(println (pop-assoc 'b L))
(println L)
(println (pop-assoc 'z L))
(println L)
(exit)
"#;
    let out = run("pop-assoc", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(l.first().copied(), Some("(b 2)"), "popped pair:\n{}", out);
    assert_eq!(
        l.get(1).copied(),
        Some("((a 1) (c 3))"),
        "list after:\n{}",
        out
    );
    assert_eq!(l.get(2).copied(), Some("nil"), "missing key:\n{}", out);
    assert_eq!(
        l.get(3).copied(),
        Some("((a 1) (c 3))"),
        "unchanged:\n{}",
        out
    );
}

#[test]
fn curry_match_filter_idiom() {
    // The Apply-and-map chapter's (filter (curry match '(a *)) …).
    let src = r#"
(println (filter (curry match '(a *)) '((a 10) (b 5) (a 3) (c 8) (a 9))))
(exit)
"#;
    let out = run("curry-match", src);
    assert!(
        out.contains("((a 10) (a 3) (a 9))"),
        "curry+match filter:\n{}",
        out
    );
}
