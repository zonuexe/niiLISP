//! niiLISP interpreter entry point.
//!
//! `niilisp <file.lsp>` runs a script; with no argument it starts a REPL.

mod builtins;
mod eval;
mod printer;
mod reader;
mod value;

use eval::{Interp, Signal};
use reader::Reader;
use value::Value;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let interp = Interp::new();

    match args.get(1) {
        Some(path) => match std::fs::read(path) {
            Ok(src) => {
                if let Err(msg) = run_source(&interp, &src) {
                    eprintln!("{}", msg);
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("niilisp: cannot read {}: {}", path, e);
                std::process::exit(1);
            }
        },
        None => repl(&interp),
    }
}

/// Read and evaluate every top-level form in `src`.
fn run_source(interp: &Interp, src: &[u8]) -> Result<(), String> {
    let forms = read_forms(interp, src)?;
    for form in &forms {
        if let Err(sig) = interp.eval(form) {
            return Err(signal_message(interp, sig));
        }
    }
    Ok(())
}

/// Read all forms, keeping the interner borrow scoped so evaluation can proceed.
fn read_forms(interp: &Interp, src: &[u8]) -> Result<Vec<Value>, String> {
    let mut interner = interp.interner.borrow_mut();
    let mut reader = Reader::new(src, &mut interner);
    reader.read_all()
}

fn signal_message(interp: &Interp, sig: Signal) -> String {
    match sig {
        Signal::Error(msg) => format!("error: {}", msg),
        Signal::Throw(v) => format!("uncaught throw: {}", interp.repr(&v)),
    }
}

fn repl(interp: &Interp) {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    loop {
        print!("niilisp> ");
        let _ = std::io::stdout().flush();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("input error: {}", e);
                break;
            }
        }

        let forms = match read_forms(interp, line.as_bytes()) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("read error: {}", e);
                continue;
            }
        };
        for form in &forms {
            match interp.eval(form) {
                Ok(v) => println!("{}", interp.repr(&v)),
                Err(sig) => eprintln!("{}", signal_message(interp, sig)),
            }
        }
    }
}
