//! External processes (ADR-0031): launch and run external programs. This half
//! is always compiled in and cross-platform — `std::process::Command` does the
//! fork+exec safely, with no `libc` or fork-in-Rust hazard. The fork-based Cilk
//! API (ADR-0032) is a separate, Unix-only, `mt`-gated slice.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::eval::{Interp, Signal};
use crate::value::Value;

fn to_millis(v: Option<&Value>) -> Result<u64, Signal> {
    match v {
        Some(Value::Int(n)) => Ok((*n).max(0) as u64),
        Some(Value::Float(f)) => Ok(f.max(0.0) as u64),
        _ => Err(Signal::error("sleep: expected a number of milliseconds")),
    }
}

fn cmd_string(v: Option<&Value>, what: &str) -> Result<String, Signal> {
    match v {
        Some(Value::Str(b)) => Ok(String::from_utf8_lossy(b).into_owned()),
        _ => Err(Signal::error(format!(
            "{}: expected a command string",
            what
        ))),
    }
}

/// `(sleep ms)` — pause for `ms` milliseconds, returning `ms`.
fn b_sleep(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let ms = to_millis(args.first())?;
    std::thread::sleep(Duration::from_millis(ms));
    Ok(Value::Int(ms as i64))
}

/// `(process "cmd arg…")` — split the command on whitespace and spawn it
/// (non-blocking), returning the child pid, or `nil` if it cannot be launched.
/// The optional stdio-redirection arguments are a deferred Unix refinement.
fn b_process(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let command = cmd_string(args.first(), "process")?;
    let mut parts = command.split_whitespace();
    let program = match parts.next() {
        Some(p) => p,
        None => return Ok(Value::Nil),
    };
    match Command::new(program).args(parts).spawn() {
        Ok(child) => Ok(Value::Int(i64::from(child.id()))),
        Err(_) => Ok(Value::Nil),
    }
}

/// Build a shell command (`sh -c cmd` on Unix, `cmd /C cmd` on Windows).
fn shell_command(command: &str) -> Command {
    #[cfg(unix)]
    {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    }
    #[cfg(not(unix))]
    {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    }
}

/// `(exec "cmd" [instr])` — run `cmd` through the shell to completion, returning
/// its stdout as a list of lines; with `instr`, feed it to the child's stdin.
fn b_exec(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let command = cmd_string(args.first(), "exec")?;
    let input = match args.get(1) {
        Some(Value::Str(b)) => Some(b.to_vec()),
        _ => None,
    };
    let mut cmd = shell_command(&command);
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return Ok(Value::Nil),
    };
    if let Some(bytes) = input {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&bytes);
        }
    }
    let mut out = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_end(&mut out);
    }
    let _ = child.wait();
    // Split into lines, dropping a single trailing newline's empty tail.
    let mut lines: Vec<&[u8]> = out.split(|&b| b == b'\n').collect();
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    Ok(Value::list(
        lines.into_iter().map(|l| Value::str(l.to_vec())).collect(),
    ))
}

/// `(! "cmd")` — run `cmd` through the shell with inherited stdio, returning its
/// exit code (or `nil` if it could not run).
fn b_shell(_: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let command = cmd_string(args.first(), "!")?;
    match shell_command(&command).status() {
        Ok(status) => Ok(Value::Int(i64::from(status.code().unwrap_or(-1)))),
        Err(_) => Ok(Value::Nil),
    }
}

pub fn install(interp: &Interp) {
    interp.register_builtin("sleep", b_sleep);
    interp.register_builtin("process", b_process);
    interp.register_builtin("exec", b_exec);
    interp.register_builtin("!", b_shell);
}
