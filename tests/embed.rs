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

// --- ADR-0040: embedding hardening ---------------------------------------

#[test]
fn exit_surfaces_as_a_signal_not_a_process_kill() {
    // `(exit)` must unwind as Signal::Exit rather than terminate this test
    // process; the host decides what to do with the code.
    let interp = Interp::new();
    match interp.eval_string(b"(exit 3)") {
        Err(Signal::Exit(3)) => {}
        other => panic!("expected Signal::Exit(3), got {other:?}"),
    }
    // A bare `(exit)` carries code 0, and the interpreter is still usable after.
    match interp.eval_string(b"(exit)") {
        Err(Signal::Exit(0)) => {}
        other => panic!("expected Signal::Exit(0), got {other:?}"),
    }
    let v = interp.eval_string(b"(+ 1 1)").unwrap();
    assert!(matches!(v, Value::Int(2)));
}

#[test]
fn exit_propagates_past_catch() {
    // A script cannot suppress its own exit by wrapping it in `catch`.
    let interp = Interp::new();
    match interp.eval_string(b"(catch (exit 7))") {
        Err(Signal::Exit(7)) => {}
        other => panic!("catch must not trap exit; got {other:?}"),
    }
}

/// A host-provided Rust primitive, registered from outside the crate.
fn host_answer(_: &Interp, _args: &[Value]) -> Result<Value, Signal> {
    Ok(Value::Int(42))
}

#[test]
fn host_can_register_a_builtin() {
    let interp = Interp::new();
    interp.register_builtin("host-answer", host_answer);
    let v = interp.eval_string(b"(+ (host-answer) 1)").unwrap();
    assert!(matches!(v, Value::Int(43)));

    // The re-exported `BuiltinFn` alias names the signature.
    let _f: niilisp::BuiltinFn = host_answer;
}

#[test]
fn eval_step_limit_stops_an_infinite_loop() {
    let interp = Interp::new();
    interp.set_eval_limit(Some(10_000));
    // An unbounded loop would never return without the limit.
    match interp.eval_string(b"(while true 1)") {
        Err(Signal::Limit) => {}
        other => panic!("expected Signal::Limit, got {other:?}"),
    }
    // The limit propagates past `catch` too.
    match interp.eval_string(b"(catch (while true 1))") {
        Err(Signal::Limit) => {}
        other => panic!("catch must not trap the step limit; got {other:?}"),
    }
    // Clearing the limit restores unbounded evaluation, and the counter resets
    // per `eval_string`, so a short program runs fine.
    interp.set_eval_limit(None);
    let v = interp.eval_string(b"(+ 2 3)").unwrap();
    assert!(matches!(v, Value::Int(5)));
}

#[test]
fn eval_limit_resets_between_runs() {
    // The counter bounds one run, not the interpreter's lifetime: many small
    // runs each under the ceiling all succeed.
    let interp = Interp::new();
    interp.set_eval_limit(Some(1_000));
    for _ in 0..5 {
        let v = interp.eval_string(b"(+ 1 1)").unwrap();
        assert!(matches!(v, Value::Int(2)));
    }
}
