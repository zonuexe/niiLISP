//! External-process tests (ADR-0031). Unix-only (they use `sh`-style commands),
//! but the builtins themselves are cross-platform.

#![cfg(unix)]

use std::process::Command;

fn run(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-proc-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("p.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn exec_captures_lines_and_stdin() {
    let src = r#"
(println "a=" (exec "printf 'x\ny\nz\n'"))
(println "b=" (exec "tr a-z A-Z" "hello"))
(println "c=" (exec "no-such-command-nii-xyz"))
(exit)
"#;
    let out = run("exec", src);
    assert!(
        out.contains(r#"a=("x" "y" "z")"#),
        "exec lines wrong:\n{}",
        out
    );
    assert!(out.contains(r#"b=("HELLO")"#), "exec stdin wrong:\n{}", out);
    assert!(out.contains("c=()"), "exec failure not empty:\n{}", out);
}

#[test]
fn shell_returns_exit_code_and_process_returns_pid() {
    let src = r#"
(println "t=" (! "true"))
(println "f=" (! "false"))
(println "p=" (if (> (process "sleep 1") 0) "pid" "no"))
(println "s=" (sleep 3))
(exit)
"#;
    let out = run("shell", src);
    assert!(out.contains("t=0"), "! true wrong:\n{}", out);
    assert!(out.contains("f=1"), "! false wrong:\n{}", out);
    assert!(out.contains("p=pid"), "process pid wrong:\n{}", out);
    assert!(out.contains("s=3"), "sleep return wrong:\n{}", out);
}

/// Binary/special-character strings must round-trip through `save`/`load` now
/// that the printer escapes re-readably (ADR-0032).
#[test]
fn string_escapes_round_trip_through_save_load() {
    let dir = std::env::temp_dir().join(format!("niilisp-esc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("s.lsp");
    let src = format!(
        r#"
(set 'orig (append "a" (char 34) (char 92) (char 10) (char 9) (char 200) "z"))
(save "{p}" 'orig)
(set 'orig nil)
(load "{p}")
(println "eq=" (= orig (append "a" (char 34) (char 92) (char 10) (char 9) (char 200) "z")))
(println "len=" (length orig))
(exit)
"#,
        p = path.display()
    );
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let script = dir.join("prog.lsp");
    std::fs::write(&script, src).unwrap();
    let out = String::from_utf8_lossy(&Command::new(bin).arg(&script).output().unwrap().stdout)
        .into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.contains("eq=true"),
        "escaped string did not round-trip:\n{}",
        out
    );
    assert!(
        out.contains("len=8"),
        "round-tripped length wrong:\n{}",
        out
    );
}
