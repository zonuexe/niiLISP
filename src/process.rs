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
    #[cfg(all(feature = "mt", unix))]
    {
        interp.register_builtin("sync", cilk::b_sync);
        interp.register_builtin("abort", cilk::b_abort);
        // `spawn` is a special form (its expression runs in the child) — see
        // `try_special_form` / `cilk::sf_spawn`.
    }
}

/// The Cilk API state (ADR-0032): the pending `spawn`ed children. Present in
/// every build so the `Interp` field type resolves, but only populated under
/// the Unix `mt` build.
#[derive(Default)]
pub struct CilkState {
    #[cfg(all(feature = "mt", unix))]
    spawns: Vec<cilk::SpawnEntry>,
}

/// `spawn`/`sync`/`abort`, forking the interpreter (ADR-0032).
#[cfg(all(feature = "mt", unix))]
pub use cilk::{sf_fork, sf_spawn};

#[cfg(all(feature = "mt", unix))]
mod cilk {
    use super::CilkState;
    use crate::eval::{Interp, Signal};
    use crate::value::{SymId, Value};
    use std::io::Read;
    use std::os::unix::io::FromRawFd;
    use std::time::{Duration, Instant};

    /// A pending `spawn`ed child: its pid, the symbol to bind its result to, and
    /// the read end of the pipe carrying that result.
    pub struct SpawnEntry {
        pid: libc::pid_t,
        sym: SymId,
        read_fd: i32,
    }

    impl CilkState {
        fn pending_pids(&self) -> Vec<Value> {
            self.spawns
                .iter()
                .map(|e| Value::Int(e.pid as i64))
                .collect()
        }
    }

    fn to_i64(v: &Value) -> i64 {
        match v {
            Value::Int(n) => *n,
            Value::Float(f) => *f as i64,
            _ => 0,
        }
    }

    /// `(spawn 'sym expr [flag])` — fork; the child evaluates `expr`, writes its
    /// `repr` to a pipe, and `_exit`s; the parent records the child and returns
    /// its pid. The result is bound to `sym` on the next `sync`.
    pub fn sf_spawn(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let sym = match interp.eval(
            args.first()
                .ok_or_else(|| Signal::error("spawn: missing symbol"))?,
        )? {
            Value::Symbol(id) => id,
            _ => return Err(Signal::error("spawn: first argument must be a symbol")),
        };
        let body = args.get(1).cloned().unwrap_or(Value::Nil);

        let mut fds = [0i32; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Ok(Value::Nil);
        }
        let (read_fd, write_fd) = (fds[0], fds[1]);

        use std::io::Write;
        let _ = std::io::stdout().flush();
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            unsafe {
                libc::close(read_fd);
                libc::close(write_fd);
            }
            return Ok(Value::Nil);
        }
        if pid == 0 {
            // Child: it manages its own (recursive) spawns, not the parent's.
            interp.cilk().borrow_mut().spawns.clear();
            unsafe { libc::close(read_fd) };
            let result = interp.eval(&body).unwrap_or(Value::Nil);
            let bytes = interp.repr(&result).into_bytes();
            let mut written = 0usize;
            while written < bytes.len() {
                let n = unsafe {
                    libc::write(
                        write_fd,
                        bytes[written..].as_ptr() as *const libc::c_void,
                        bytes.len() - written,
                    )
                };
                if n <= 0 {
                    break;
                }
                written += n as usize;
            }
            unsafe { libc::close(write_fd) };
            let _ = std::io::stdout().flush();
            unsafe { libc::_exit(0) };
        }
        // Parent.
        unsafe { libc::close(write_fd) };
        interp
            .cilk()
            .borrow_mut()
            .spawns
            .push(SpawnEntry { pid, sym, read_fd });
        Ok(Value::Int(pid as i64))
    }

    /// `(fork expr)` — fork; the child evaluates `expr` and `_exit`s (no result
    /// is returned); the parent gets the child pid, or `nil` on failure.
    pub fn sf_fork(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let body = args.first().cloned().unwrap_or(Value::Nil);
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Ok(Value::Nil);
        }
        if pid == 0 {
            interp.cilk().borrow_mut().spawns.clear();
            let _ = interp.eval(&body);
            let _ = std::io::stdout().flush();
            unsafe { libc::_exit(0) };
        }
        Ok(Value::Int(pid as i64))
    }

    /// `(sync)` returns the pending pids. `(sync timeout [inlet])` waits up to
    /// `timeout` ms, binding each finished child's result to its symbol (and
    /// calling `(inlet pid)` if given); returns `true` if all finished else `nil`.
    pub fn b_sync(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        if args.is_empty() {
            return Ok(Value::list(interp.cilk().borrow().pending_pids()));
        }
        let timeout = to_i64(&args[0]).max(0) as u64;
        let inlet = args
            .get(1)
            .cloned()
            .filter(|v| matches!(v, Value::Lambda(_) | Value::Fexpr(_) | Value::Builtin(_)));
        let deadline = Instant::now() + Duration::from_millis(timeout);

        loop {
            let pending: Vec<(libc::pid_t, i32, SymId)> = interp
                .cilk()
                .borrow()
                .spawns
                .iter()
                .map(|e| (e.pid, e.read_fd, e.sym))
                .collect();
            if pending.is_empty() {
                break;
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remaining = (deadline - now).as_millis().min(i32::MAX as u128) as i32;
            let mut pfds: Vec<libc::pollfd> = pending
                .iter()
                .map(|&(_, fd, _)| libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                })
                .collect();
            let n = unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, remaining) };
            if n < 0 {
                break;
            }
            if n == 0 {
                break; // timed out
            }
            for (i, pfd) in pfds.iter().enumerate() {
                if pfd.revents & (libc::POLLIN | libc::POLLHUP) == 0 {
                    continue;
                }
                let (pid, fd, sym) = pending[i];
                // The child writes its whole result then closes, so read to EOF.
                let mut buf = Vec::new();
                {
                    let mut f = unsafe { std::fs::File::from_raw_fd(fd) };
                    let _ = f.read_to_end(&mut buf);
                } // f drops -> closes fd
                let mut status = 0i32;
                unsafe { libc::waitpid(pid, &mut status, 0) };
                let value = interp.read_one(&buf);
                interp.set_global(sym, value);
                interp.cilk().borrow_mut().spawns.retain(|e| e.pid != pid);
                if let Some(f) = &inlet {
                    interp.call(f, vec![Value::Int(pid as i64)])?;
                }
            }
        }
        Ok(if interp.cilk().borrow().spawns.is_empty() {
            Value::True
        } else {
            Value::Nil
        })
    }

    /// `(abort pid)` kills and reaps one child; `(abort)` kills all pending
    /// children. Returns `true`.
    pub fn b_abort(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let targets: Vec<(libc::pid_t, i32)> = {
            let state = interp.cilk().borrow();
            match args.first() {
                Some(v) => {
                    let pid = to_i64(v) as libc::pid_t;
                    state
                        .spawns
                        .iter()
                        .filter(|e| e.pid == pid)
                        .map(|e| (e.pid, e.read_fd))
                        .collect()
                }
                None => state.spawns.iter().map(|e| (e.pid, e.read_fd)).collect(),
            }
        };
        for (pid, fd) in &targets {
            unsafe {
                libc::kill(*pid, libc::SIGKILL);
                let mut status = 0i32;
                libc::waitpid(*pid, &mut status, 0);
                libc::close(*fd);
            }
        }
        let killed: std::collections::HashSet<libc::pid_t> =
            targets.iter().map(|(p, _)| *p).collect();
        interp
            .cilk()
            .borrow_mut()
            .spawns
            .retain(|e| !killed.contains(&e.pid));
        Ok(Value::True)
    }
}
