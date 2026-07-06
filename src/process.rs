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
        interp.register_builtin("share", cilk::b_share);
        interp.register_builtin("pipe", cilk::b_pipe);
        interp.register_builtin("wait-pid", cilk::b_wait_pid);
        interp.register_builtin("send", cilk::b_send);
        interp.register_builtin("signal", cilk::b_signal);
        // `receive` is a place-taking special form (see `try_special_form`).
        // `spawn` is a special form (its expression runs in the child) — see
        // `try_special_form` / `cilk::sf_spawn`.
    }
}

/// The Cilk API state (ADR-0032): the pending `spawn`ed children, `share`d
/// pages, and message channels. Only exists in the Unix `mt` build (the `Interp`
/// field that holds it is `mt`-gated too).
#[cfg(all(feature = "mt", unix))]
#[derive(Default)]
pub struct CilkState {
    spawns: Vec<cilk::SpawnEntry>,
    pages: Vec<cilk::SharePage>,
    /// Message channels: for each peer process (a child in the parent, the
    /// parent in a child), the non-blocking datagram socket to it (ADR-0032).
    channels: Vec<(libc::pid_t, i32)>,
}

/// `spawn`/`sync`/`abort`, forking the interpreter (ADR-0032).
#[cfg(all(feature = "mt", unix))]
pub use cilk::{dispatch_signals, receive_one, receive_ready, sf_fork, sf_spawn};

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

    /// A `share`d memory page: an `mmap`ed `MAP_SHARED` region, inherited by
    /// children across `fork` (ADR-0032).
    pub struct SharePage {
        addr: usize,
    }

    /// One `share` page. A page holds `[u32 length][repr bytes]`; 4 KiB is far
    /// more than the small serialised values `share` carries.
    const SHARE_PAGE_SIZE: usize = 4096;

    /// `(share)` allocates a shared page and returns its address; `(share adr
    /// val)` writes `val` (as its binary-safe `repr`); `(share adr)` reads it
    /// back (`nil` if never written). The address is validated against the page
    /// registry (bounding the `unsafe`); children inherit both the mapping and
    /// the registry across `fork`.
    pub fn b_share(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let adr = match args.first() {
            None => {
                let addr = unsafe {
                    libc::mmap(
                        std::ptr::null_mut(),
                        SHARE_PAGE_SIZE,
                        libc::PROT_READ | libc::PROT_WRITE,
                        libc::MAP_SHARED | libc::MAP_ANONYMOUS,
                        -1,
                        0,
                    )
                };
                if addr == libc::MAP_FAILED {
                    return Ok(Value::Nil);
                }
                unsafe { std::ptr::write_bytes(addr as *mut u8, 0, 4) };
                let a = addr as usize;
                interp.cilk().borrow_mut().pages.push(SharePage { addr: a });
                return Ok(Value::Int(a as i64));
            }
            Some(v) => to_i64(v) as usize,
        };

        if !interp.cilk().borrow().pages.iter().any(|p| p.addr == adr) {
            return Err(Signal::error("share: address is not a shared page"));
        }
        let ptr = adr as *mut u8;

        if let Some(val) = args.get(1) {
            let bytes = interp.repr(val).into_bytes();
            if bytes.len() + 4 > SHARE_PAGE_SIZE {
                return Err(Signal::error("share: value too large for the page"));
            }
            unsafe {
                std::ptr::copy_nonoverlapping((bytes.len() as u32).to_ne_bytes().as_ptr(), ptr, 4);
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.add(4), bytes.len());
            }
            Ok(val.clone())
        } else {
            let len = unsafe {
                let mut lb = [0u8; 4];
                std::ptr::copy_nonoverlapping(ptr, lb.as_mut_ptr(), 4);
                u32::from_ne_bytes(lb) as usize
            };
            if len == 0 || len + 4 > SHARE_PAGE_SIZE {
                return Ok(Value::Nil);
            }
            let mut buf = vec![0u8; len];
            unsafe { std::ptr::copy_nonoverlapping(ptr.add(4), buf.as_mut_ptr(), len) };
            Ok(interp.read_one(&buf))
        }
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
        // A truthy third argument enables the send/receive message channel.
        let message = match args.get(2) {
            Some(e) => interp.eval(e)?.is_truthy(),
            None => false,
        };

        let mut pipe_fds = [0i32; 2];
        if unsafe { libc::pipe(pipe_fds.as_mut_ptr()) } != 0 {
            return Ok(Value::Nil);
        }
        let (read_fd, write_fd) = (pipe_fds[0], pipe_fds[1]);

        // A non-blocking datagram socketpair for messages, if requested.
        let mut sv = [0i32; 2];
        if message
            && unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_DGRAM, 0, sv.as_mut_ptr()) } != 0
        {
            unsafe {
                libc::close(read_fd);
                libc::close(write_fd);
            }
            return Ok(Value::Nil);
        }

        use std::io::Write;
        let _ = std::io::stdout().flush();
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            unsafe {
                libc::close(read_fd);
                libc::close(write_fd);
                if message {
                    libc::close(sv[0]);
                    libc::close(sv[1]);
                }
            }
            return Ok(Value::Nil);
        }
        if pid == 0 {
            // Child: it manages its own (recursive) spawns and channels.
            {
                let mut state = interp.cilk().borrow_mut();
                state.spawns.clear();
                state.channels.clear();
                if message {
                    unsafe {
                        libc::close(sv[0]);
                        set_nonblocking(sv[1]);
                    }
                    state.channels.push((unsafe { libc::getppid() }, sv[1]));
                }
            }
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
        let mut state = interp.cilk().borrow_mut();
        state.spawns.push(SpawnEntry { pid, sym, read_fd });
        if message {
            unsafe {
                libc::close(sv[1]);
                set_nonblocking(sv[0]);
            }
            state.channels.push((pid, sv[0]));
        }
        Ok(Value::Int(pid as i64))
    }

    unsafe fn set_nonblocking(fd: i32) {
        let flags = libc::fcntl(fd, libc::F_GETFL, 0);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        // Widen the socket buffers so larger datagrams fit (a SOCK_DGRAM message
        // must fit the buffer). Very large messages (qa-msgbig's 80 KB) still
        // exceed this and would need stream framing — deferred.
        let sz: libc::c_int = 262_144;
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_SNDBUF,
            &sz as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVBUF,
            &sz as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }

    /// `(send pid msg)` — send `msg` (as its `repr`) as one datagram to the peer
    /// `pid`; `true` on success, `nil` if the buffer is full (the caller retries)
    /// or there is no such peer.
    pub fn b_send(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let pid = to_i64(args.first().unwrap_or(&Value::Nil)) as libc::pid_t;
        let msg = args.get(1).cloned().unwrap_or(Value::Nil);
        let fd = interp
            .cilk()
            .borrow()
            .channels
            .iter()
            .find(|(p, _)| *p == pid)
            .map(|(_, fd)| *fd);
        let fd = match fd {
            Some(fd) => fd,
            None => return Ok(Value::Nil),
        };
        let bytes = interp.repr(&msg).into_bytes();
        let n = unsafe {
            libc::send(
                fd,
                bytes.as_ptr() as *const libc::c_void,
                bytes.len(),
                libc::MSG_DONTWAIT,
            )
        };
        Ok(if n == bytes.len() as isize {
            Value::True
        } else {
            Value::Nil
        })
    }

    /// `(receive)` — the peer pids with a datagram ready to read.
    pub fn receive_ready(interp: &Interp) -> Vec<Value> {
        let channels = interp.cilk().borrow().channels.clone();
        let mut ready = Vec::new();
        for (pid, fd) in channels {
            let mut pfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            if unsafe { libc::poll(&mut pfd, 1, 0) } > 0 && pfd.revents & libc::POLLIN != 0 {
                ready.push(Value::Int(pid as i64));
            }
        }
        ready
    }

    /// Read one datagram from peer `pid` (non-blocking); `None` if no peer or no
    /// message is waiting. The `receive` special form binds the value to a place.
    pub fn receive_one(interp: &Interp, pid: i64) -> Option<Value> {
        let pid = pid as libc::pid_t;
        let fd = interp
            .cilk()
            .borrow()
            .channels
            .iter()
            .find(|(p, _)| *p == pid)
            .map(|(_, fd)| *fd)?;
        let mut buf = vec![0u8; 65536];
        let n = unsafe {
            libc::recv(
                fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                libc::MSG_DONTWAIT,
            )
        };
        if n <= 0 {
            return None;
        }
        buf.truncate(n as usize);
        Some(interp.read_one(&buf))
    }

    use std::sync::atomic::{AtomicU64, Ordering};

    /// Bitmask of OS signals delivered since the last dispatch (bit = signal
    /// number). Set from the async signal handler — only an atomic `fetch_or`,
    /// which is async-signal-safe (ADR-0032).
    static PENDING: AtomicU64 = AtomicU64::new(0);

    extern "C" fn os_signal_handler(sig: libc::c_int) {
        if (0..64).contains(&sig) {
            PENDING.fetch_or(1u64 << sig, Ordering::Relaxed);
        }
    }

    /// `(signal n handler)` — run `handler` (a function of the signal number)
    /// when OS signal `n` arrives; a nil handler restores the default. Returns
    /// the previous handler. The handler runs at the next eval safe point, not
    /// in async signal context.
    pub fn b_signal(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let n = to_i64(args.first().unwrap_or(&Value::Nil));
        if !(0..64).contains(&n) {
            return Err(Signal::error("signal: number out of range"));
        }
        let handler = args.get(1).cloned().unwrap_or(Value::Nil);
        if matches!(handler, Value::Nil) {
            interp.signal_handlers().borrow_mut().remove(&(n as i32));
            unsafe { libc::signal(n as libc::c_int, libc::SIG_DFL) };
            Ok(Value::Nil)
        } else {
            let prev = interp
                .signal_handlers()
                .borrow_mut()
                .insert(n as i32, handler);
            unsafe {
                libc::signal(
                    n as libc::c_int,
                    os_signal_handler as *const () as usize as libc::sighandler_t,
                )
            };
            Ok(prev.unwrap_or(Value::Nil))
        }
    }

    /// Run any pending OS signal handlers (ADR-0032) — a safe point called from
    /// `eval_body`. The fast path is a single relaxed load. Handler errors are
    /// swallowed so an async signal never disrupts the main evaluation.
    pub fn dispatch_signals(interp: &Interp) {
        let mask = PENDING.swap(0, Ordering::Relaxed);
        if mask == 0 {
            return;
        }
        for sig in 0..64i32 {
            if mask & (1u64 << sig) != 0 {
                let handler = interp.signal_handlers().borrow().get(&sig).cloned();
                if let Some(h) = handler {
                    let _ = interp.call(&h, vec![Value::Int(i64::from(sig))]);
                }
            }
        }
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

    /// `(pipe)` — create an OS pipe, register both ends as file handles
    /// (ADR-0032), and return `(read-handle write-handle)`. The handles work
    /// with `read-line`/`write-line`/`close` and are inherited across `fork`.
    pub fn b_pipe(interp: &Interp, _args: &[Value]) -> Result<Value, Signal> {
        let mut fds = [0i32; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Ok(Value::Nil);
        }
        let mut tbl = interp.files().borrow_mut();
        let read_h = tbl.insert_fd(fds[0]);
        let write_h = tbl.insert_fd(fds[1]);
        Ok(Value::list(vec![Value::Int(read_h), Value::Int(write_h)]))
    }

    /// `(wait-pid pid)` — wait for child `pid` to terminate, returning its pid
    /// (or `nil` on error). `(wait-pid pid true)` polls without blocking.
    pub fn b_wait_pid(_interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let pid = to_i64(args.first().unwrap_or(&Value::Nil)) as libc::pid_t;
        let flags = if matches!(args.get(1), Some(v) if v.is_truthy()) {
            libc::WNOHANG
        } else {
            0
        };
        let mut status = 0i32;
        let r = unsafe { libc::waitpid(pid, &mut status, flags) };
        if r < 0 {
            Ok(Value::Nil)
        } else {
            Ok(Value::Int(r as i64))
        }
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
        let mut state = interp.cilk().borrow_mut();
        state.spawns.retain(|e| !killed.contains(&e.pid));
        // Drop and close any message channels to the killed children.
        state.channels.retain(|(pid, fd)| {
            if killed.contains(pid) {
                unsafe { libc::close(*fd) };
                false
            } else {
                true
            }
        });
        Ok(Value::True)
    }
}
