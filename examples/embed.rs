//! Embedding niiLISP as an in-process interpreter (ADR-0039).
//!
//! Run with: `cargo run --example embed`
//! (or, for a sandboxed interpreter, `cargo run --no-default-features --example embed`).

use niilisp::Interp;

fn main() {
    // One interpreter owns all state. It is single-threaded (`!Send`/`!Sync`),
    // so keep it on one thread.
    let interp = Interp::new();

    // `eval_string` reads every form in the source and returns the last value.
    // Bindings persist across calls on the same interpreter.
    interp.eval_string(b"(set 'greeting \"hello\")").unwrap();
    interp.eval_string(b"(define (square x) (* x x))").unwrap();

    // Read a value back and render it with `repr`.
    let v = interp.eval_string(b"(list greeting (square 9))").unwrap();
    println!("result: {}", interp.repr(&v)); // => ("hello" 81)

    // Errors (and uncaught `throw`s) come back as `Err(Signal)`.
    match interp.eval_string(b"(+ 1 unbound-fn-call)") {
        Ok(v) => println!("ok: {}", interp.repr(&v)),
        Err(e) => println!("error: {}", niilisp::signal_message(&interp, e)),
    }

    // Pass structured data in as a niiLISP expression, compute, and read it out.
    let sum = interp.eval_string(b"(apply + (sequence 1 100))").unwrap();
    println!("1..100 sum: {}", interp.repr(&sum)); // => 5050
}
