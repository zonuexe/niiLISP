//! newLISP-faithful edge semantics for a few builtins that the WikiBook
//! coverage audit found diverging: `int` (nil/default on failure, base and
//! prefix parsing), `dup`'s list-flag, and the 1-arg / folded shift operators.

use std::process::Command;

fn eval(expr: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let out = Command::new(bin).arg("-e").arg(expr).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn int_returns_nil_or_default_on_failure() {
    assert_eq!(eval("(println (int \"x\"))"), "nil\n");
    assert_eq!(eval("(println (int \"x\" 0))"), "0\n");
    assert_eq!(eval("(println (int \"x\" 42))"), "42\n");
    // A leading numeric run / float string still parses.
    assert_eq!(eval("(println (int \"3.7\"))"), "3\n");
    assert_eq!(eval("(println (int \"12abc\"))"), "12\n");
}

#[test]
fn int_parses_base_and_prefixes() {
    assert_eq!(eval("(println (int \"0x1F\"))"), "31\n");
    assert_eq!(eval("(println (int \"0b1010\"))"), "10\n");
    assert_eq!(eval("(println (int \"FF\" 0 16))"), "255\n");
    // Explicit base 10 overrides a leading-zero string.
    assert_eq!(eval("(println (int \"08\" 0 10))"), "8\n");
}

#[test]
fn dup_list_flag() {
    assert_eq!(eval("(println (dup \"x\" 3))"), "xxx\n");
    assert_eq!(
        eval("(println (dup \"x\" 3 true))"),
        "(\"x\" \"x\" \"x\")\n"
    );
}

#[test]
fn shift_one_arg_and_fold() {
    assert_eq!(eval("(println (<< 6))"), "12\n");
    assert_eq!(eval("(println (>> 6))"), "3\n");
    assert_eq!(eval("(println (<< 6 1))"), "12\n");
    assert_eq!(eval("(println (<< 1 2 3))"), "32\n"); // 1<<2<<3
}
