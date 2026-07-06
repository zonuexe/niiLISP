//! The `$idx` system iterator variable, which newLISP maintains inside
//! `dolist`/`dostring`/`dotree`/`map` and the `while`/`until`/`do-while`/
//! `do-until` loops. It must reflect the current 0-based offset and be
//! restored (to its prior value, or unbound) when the loop exits.

use std::process::Command;

fn run(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-idx-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("p.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn idx_in_dolist_and_map() {
    // Matches the WikiBook examples verbatim.
    let src = r#"
(dolist (x '(a b d e f g)) (println $idx ":" x))
(println (map (fn (x) (list $idx x)) '(a b c)))
(exit)
"#;
    let out = run("dolist-map", src);
    assert!(out.contains("0:a"), "dolist $idx wrong:\n{}", out);
    assert!(out.contains("5:g"), "dolist $idx wrong:\n{}", out);
    assert!(
        out.contains("((0 a) (1 b) (2 c))"),
        "map $idx wrong:\n{}",
        out
    );
}

#[test]
fn idx_counts_while_and_dostring() {
    let src = r#"
(set 'x 0)
(while (< x 3) (print $idx) (inc x))
(println)
(dostring (c "AB") (println $idx ":" c))
(exit)
"#;
    let out = run("while-dostring", src);
    assert!(out.contains("012"), "while $idx wrong:\n{}", out);
    assert!(out.contains("0:65"), "dostring $idx wrong:\n{}", out);
    assert!(out.contains("1:66"), "dostring $idx wrong:\n{}", out);
}

#[test]
fn idx_is_restored_after_nested_loops() {
    // The outer loop's $idx must survive an inner loop that also sets it,
    // and $idx must read back as nil once no loop is active.
    let src = r#"
(dolist (x '(a b))
  (map (fn (y) y) '(1 2 3))
  (println "outer" $idx))
(println "after=" $idx)
(exit)
"#;
    let out = run("nested", src);
    assert!(out.contains("outer0"), "nested restore wrong:\n{}", out);
    assert!(out.contains("outer1"), "nested restore wrong:\n{}", out);
    assert!(out.contains("after=nil"), "$idx not restored:\n{}", out);
}
