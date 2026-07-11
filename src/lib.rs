//! niiLISP — a re-implementation of the newLISP dialect, usable as an embedded
//! interpreter (ADR-0039).
//!
//! Create an [`Interp`], feed it source, and read back a [`Value`]:
//!
//! ```
//! let interp = niilisp::Interp::new();
//! match interp.eval_string(b"(+ 1 2)") {
//!     Ok(v) => assert_eq!(interp.repr(&v), "3"),
//!     Err(e) => panic!("{e:?}"),
//! }
//! ```
//!
//! # Caveats when embedding
//!
//! - **`(exit)` terminates the host process** (it calls [`std::process::exit`]).
//!   Avoid running untrusted scripts that may call it.
//! - **Single-threaded.** [`Interp`] is built on `Rc`/`RefCell`, so it is neither
//!   `Send` nor `Sync`; use one interpreter per thread.
//! - **Default features touch the host OS.** The default build enables `mt`
//!   (real `fork()` of the host for `spawn`/`process`), `net`, and `ffi`. For a
//!   sandboxed interpreter, depend with `default-features = false` and opt into
//!   only what you need (e.g. `bigint`, `regex`, `date`).
//!
//! The `0.x` API is unstable; pin an exact version.

// The interpreter core. `Interp`, `Value`, and `Signal` are re-exported below as
// the intended embedding surface; the modules themselves are implementation
// detail and not covered by any stability promise.
#[doc(hidden)]
pub mod eval;
#[doc(hidden)]
pub mod reader;
#[doc(hidden)]
pub mod repl;
#[doc(hidden)]
pub mod value;

mod builtins;
mod date;
mod ffi;
mod fileio;
mod json;
mod net;
mod printer;
mod process;
mod utf8;
mod xml;

pub use eval::{Interp, Signal};
pub use value::Value;

/// Read every top-level form from `src` (no evaluation). A CLI/REPL helper
/// shared by the binary and the REPL; not part of the stable embedding surface
/// (embedders use [`Interp::eval_string`]).
#[doc(hidden)]
pub fn read_forms(interp: &Interp, src: &[u8]) -> Result<Vec<Value>, String> {
    // Collect the MAIN primitive names before borrowing the interner (ADR-0026).
    let primitives = interp.primitive_names();
    let mut interner = interp.interner.borrow_mut();
    let mut reader = reader::Reader::new(src, &mut interner, &primitives);
    reader.read_all()
}

/// Format a [`Signal`] (a runtime error or an uncaught `throw`) for display.
#[doc(hidden)]
pub fn signal_message(interp: &Interp, sig: Signal) -> String {
    match sig {
        Signal::Error(msg) => msg,
        Signal::Throw(v) => format!("uncaught throw: {}", interp.repr(&v)),
    }
}
