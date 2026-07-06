//! `context` reflection and symbol-creation forms (ADR-0026):
//! `(context)` returns the current context; `(context ctx word [value])`
//! creates `ctx:word` and optionally sets it, returning the symbol.

use std::process::Command;

fn run(src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let out = Command::new(bin).arg("-e").arg(src).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn context_no_arg_returns_current() {
    assert_eq!(run("(println (context))"), "MAIN\n");
    // Switching updates it; switching back restores MAIN.
    assert_eq!(
        run("(context 'FOO)(println (context))(context MAIN)(println (context))"),
        "FOO\nMAIN\n"
    );
}

#[test]
fn context_creates_and_sets_symbols() {
    // 4-arg form sets ctx:word; the symbol is reachable by qualified name.
    assert_eq!(
        run("(context 'D 'a 1)(context 'D 'b 2)(println D:a \" \" D:b)"),
        "1 2\n"
    );
    // String word form.
    assert_eq!(run("(context 'F \"key\" 99)(println F:key)"), "99\n");
    // 3-arg form (no value) returns the created symbol.
    assert_eq!(run("(println (context 'E 'x))"), "E:x\n");
}
