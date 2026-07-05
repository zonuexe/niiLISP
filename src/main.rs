//! niiLISP interpreter command-line entry point.
//!
//! Usage:
//!
//! ```text
//! niilisp FILE          run a script (or start a REPL if no FILE)
//! niilisp -e EXPR       evaluate EXPR
//! niilisp -             read a script from standard input
//! niilisp --version     print the version
//! niilisp --help        print usage
//! ```

mod builtins;
mod eval;
mod ffi;
mod printer;
mod reader;
mod value;

use std::io::Read;
use std::process::ExitCode;

use eval::{Interp, Signal};
use reader::Reader;
use value::Value;

const USAGE: &str = "\
niilisp - a re-implementation of the newLISP dialect

USAGE:
    niilisp [FILE]        Run a script file
    niilisp -e EXPR       Evaluate an expression
    niilisp -             Read and run a script from standard input
    niilisp               Start an interactive REPL

OPTIONS:
    -e EXPR              Evaluate EXPR and exit
    -h, --help          Print this help
    -V, --version       Print version
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let interp = Interp::new();
    interp.set_main_args(args.clone());

    match args.get(1).map(String::as_str) {
        None => {
            repl(&interp);
            ExitCode::SUCCESS
        }
        Some("-h") | Some("--help") => {
            print!("{}", USAGE);
            ExitCode::SUCCESS
        }
        Some("-V") | Some("--version") => {
            println!("niilisp {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("-e") => match args.get(2) {
            Some(expr) => run_or_exit(&interp, expr.as_bytes()),
            None => {
                eprintln!("niilisp: -e requires an expression");
                ExitCode::from(2)
            }
        },
        Some("-") => {
            let mut src = Vec::new();
            if let Err(e) = std::io::stdin().read_to_end(&mut src) {
                eprintln!("niilisp: cannot read stdin: {}", e);
                return ExitCode::from(2);
            }
            run_or_exit(&interp, &src)
        }
        Some(flag) if flag.starts_with('-') => {
            eprintln!("niilisp: unknown option '{}'\n\n{}", flag, USAGE);
            ExitCode::from(2)
        }
        Some(path) => match std::fs::read(path) {
            Ok(src) => run_or_exit(&interp, &src),
            Err(e) => {
                eprintln!("niilisp: cannot read {}: {}", path, e);
                ExitCode::from(2)
            }
        },
    }
}

/// Run a whole source, reporting a runtime error to stderr with exit code 1.
fn run_or_exit(interp: &Interp, src: &[u8]) -> ExitCode {
    match run_source(interp, src) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("niilisp: {}", msg);
            ExitCode::FAILURE
        }
    }
}

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
        Signal::Error(msg) => msg,
        Signal::Throw(v) => format!("uncaught throw: {}", interp.repr(&v)),
    }
}

fn repl(interp: &Interp) {
    use std::io::{BufRead, Write};
    eprintln!(
        "niilisp {} - type expressions, Ctrl-D to exit",
        env!("CARGO_PKG_VERSION")
    );
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
