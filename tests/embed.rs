//! The embedding library API (ADR-0039): `niilisp::{Interp, Value, Signal}`.
//! Unlike the other integration tests, this one links the crate as a *library*
//! and calls it directly — no subprocess.

use niilisp::{Interp, Signal, Value};

#[test]
fn eval_string_returns_the_last_value() {
    let interp = Interp::new();
    let v = interp.eval_string(b"(+ 1 2) (* 3 4)").unwrap();
    assert_eq!(interp.repr(&v), "12");
}

#[test]
fn bindings_persist_across_calls() {
    let interp = Interp::new();
    interp.eval_string(b"(set 'x 40)").unwrap();
    let v = interp.eval_string(b"(+ x 2)").unwrap();
    assert!(matches!(v, Value::Int(42)));
}

#[test]
fn errors_come_back_as_signal() {
    let interp = Interp::new();
    // An uncaught throw surfaces as Signal::Throw.
    match interp.eval_string(b"(throw 99)") {
        Err(Signal::Throw(v)) => assert!(matches!(v, Value::Int(99))),
        other => panic!("expected a throw, got {other:?}"),
    }
    // A runtime error surfaces as Signal::Error with a message.
    match interp.eval_string(b"(foo-not-a-fn 1)") {
        Err(e @ Signal::Error(_)) => {
            assert!(niilisp::signal_message(&interp, e).contains("not a function"));
        }
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn repr_renders_values() {
    let interp = Interp::new();
    let v = interp
        .eval_string(br#"(list 1 "two" (sequence 3 5))"#)
        .unwrap();
    assert_eq!(interp.repr(&v), r#"(1 "two" (3 4 5))"#);
}
