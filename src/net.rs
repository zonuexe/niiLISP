//! Networking: `net-*` stream sockets (ADR-0033). Unix-only, behind the `net`
//! feature. A socket is a `FileTable` handle (a raw fd), so `net-send` /
//! `net-receive` / `net-close` reuse the file-I/O machinery; only socket
//! creation and `net-accept`/`net-select`/`net-peek`/`net-peer`/`net-local`
//! touch `libc` directly. `net-receive` is the `read-buffer` special form.

#[cfg(all(feature = "net", unix))]
pub use imp::install;

#[cfg(not(all(feature = "net", unix)))]
pub fn install(_interp: &crate::eval::Interp) {}

#[cfg(all(feature = "net", unix))]
mod imp {
    use std::net::{TcpListener, TcpStream};
    use std::os::unix::io::IntoRawFd;
    use std::os::unix::net::{UnixListener, UnixStream};

    use crate::eval::{Interp, Signal};
    use crate::value::Value;

    fn to_i64(v: &Value) -> i64 {
        match v {
            Value::Int(n) => *n,
            Value::Float(f) => *f as i64,
            _ => 0,
        }
    }

    fn register(interp: &Interp, fd: Option<i32>) -> Value {
        match fd {
            Some(fd) => Value::Int(interp.files().borrow_mut().insert_fd(fd)),
            None => Value::Nil,
        }
    }

    /// `(net-connect host port)` (TCP) or `(net-connect "/path")` (Unix domain).
    fn b_net_connect(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let fd = match (args.first(), args.get(1)) {
            (Some(Value::Str(host)), Some(port)) => {
                let addr = format!("{}:{}", String::from_utf8_lossy(host), to_i64(port));
                TcpStream::connect(addr).ok().map(IntoRawFd::into_raw_fd)
            }
            (Some(Value::Str(path)), None) => {
                UnixStream::connect(String::from_utf8_lossy(path).as_ref())
                    .ok()
                    .map(IntoRawFd::into_raw_fd)
            }
            _ => return Err(Signal::error("net-connect: expected (host port) or (path)")),
        };
        Ok(register(interp, fd))
    }

    /// `(net-listen port)` (TCP) or `(net-listen "/path")` (Unix domain).
    fn b_net_listen(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let fd = match args.first() {
            Some(Value::Int(port)) => TcpListener::bind(("0.0.0.0", *port as u16))
                .ok()
                .map(IntoRawFd::into_raw_fd),
            Some(Value::Str(path)) => {
                let p = String::from_utf8_lossy(path).into_owned();
                let _ = std::fs::remove_file(&p); // clear a stale socket file
                UnixListener::bind(&p).ok().map(IntoRawFd::into_raw_fd)
            }
            _ => return Err(Signal::error("net-listen: expected a port or path")),
        };
        Ok(register(interp, fd))
    }

    /// `(net-accept lsock)` — accept one connection (blocking).
    fn b_net_accept(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let h = to_i64(args.first().unwrap_or(&Value::Nil));
        let lfd = match interp.files().borrow().raw_fd(h) {
            Some(fd) => fd,
            None => return Ok(Value::Nil),
        };
        let new_fd = unsafe { libc::accept(lfd, std::ptr::null_mut(), std::ptr::null_mut()) };
        if new_fd < 0 {
            return Ok(Value::Nil);
        }
        Ok(Value::Int(interp.files().borrow_mut().insert_fd(new_fd)))
    }

    /// `(net-send sock str)` — send bytes; returns the count.
    fn b_net_send(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let h = to_i64(args.first().unwrap_or(&Value::Nil));
        let data = match args.get(1) {
            Some(Value::Str(b)) => b.clone(),
            _ => return Err(Signal::error("net-send: expected a string")),
        };
        Ok(crate::fileio::write_handle(interp, h, &data).map_or(Value::Nil, Value::Int))
    }

    /// `(net-select sock "read"|"write" timeout-ms)` — poll one socket.
    fn b_net_select(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let sock = args.first().cloned().unwrap_or(Value::Nil);
        let fd = match interp.files().borrow().raw_fd(to_i64(&sock)) {
            Some(fd) => fd,
            None => return Ok(Value::Nil),
        };
        let write = matches!(args.get(1), Some(Value::Str(b)) if b.first() == Some(&b'w'));
        let events = if write { libc::POLLOUT } else { libc::POLLIN };
        let timeout = to_i64(args.get(2).unwrap_or(&Value::Nil)) as i32;
        let mut pfd = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let n = unsafe { libc::poll(&mut pfd, 1, timeout) };
        Ok(if n > 0 && pfd.revents & events != 0 {
            sock
        } else {
            Value::Nil
        })
    }

    /// `(net-peek sock)` — bytes available to read.
    fn b_net_peek(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let h = to_i64(args.first().unwrap_or(&Value::Nil));
        let fd = match interp.files().borrow().raw_fd(h) {
            Some(fd) => fd,
            None => return Ok(Value::Nil),
        };
        let mut n: libc::c_int = 0;
        if unsafe { libc::ioctl(fd, libc::FIONREAD, &mut n) } < 0 {
            return Ok(Value::Nil);
        }
        Ok(Value::Int(i64::from(n)))
    }

    /// The remote (`peer`) or local address of a socket as a best-effort string
    /// (`ip:port` for TCP, empty for a Unix-domain / unnamed socket).
    fn addr_string(fd: i32, peer: bool) -> String {
        let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        let sa = std::ptr::addr_of_mut!(storage) as *mut libc::sockaddr;
        let r = unsafe {
            if peer {
                libc::getpeername(fd, sa, &mut len)
            } else {
                libc::getsockname(fd, sa, &mut len)
            }
        };
        if r < 0 {
            return String::new();
        }
        if i32::from(storage.ss_family) == libc::AF_INET {
            let a = unsafe { &*(std::ptr::addr_of!(storage) as *const libc::sockaddr_in) };
            let ip = u32::from_be(a.sin_addr.s_addr);
            let port = u16::from_be(a.sin_port);
            format!(
                "{}.{}.{}.{}:{}",
                (ip >> 24) & 0xff,
                (ip >> 16) & 0xff,
                (ip >> 8) & 0xff,
                ip & 0xff,
                port
            )
        } else {
            String::new()
        }
    }

    fn b_net_peer(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let h = to_i64(args.first().unwrap_or(&Value::Nil));
        match interp.files().borrow().raw_fd(h) {
            Some(fd) => Ok(Value::str(addr_string(fd, true).into_bytes())),
            None => Ok(Value::Nil),
        }
    }

    fn b_net_local(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let h = to_i64(args.first().unwrap_or(&Value::Nil));
        match interp.files().borrow().raw_fd(h) {
            Some(fd) => Ok(Value::str(addr_string(fd, false).into_bytes())),
            None => Ok(Value::Nil),
        }
    }

    /// `(net-close sock)` — close the socket handle.
    fn b_net_close(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
        let h = to_i64(args.first().unwrap_or(&Value::Nil));
        Ok(if crate::fileio::close_handle(interp, h) {
            Value::True
        } else {
            Value::Nil
        })
    }

    pub fn install(interp: &Interp) {
        interp.register_builtin("net-connect", b_net_connect);
        interp.register_builtin("net-listen", b_net_listen);
        interp.register_builtin("net-accept", b_net_accept);
        interp.register_builtin("net-send", b_net_send);
        interp.register_builtin("net-select", b_net_select);
        interp.register_builtin("net-peek", b_net_peek);
        interp.register_builtin("net-peer", b_net_peer);
        interp.register_builtin("net-local", b_net_local);
        interp.register_builtin("net-close", b_net_close);
        // `net-receive` is the `read-buffer` special form (see `try_special_form`).
    }
}
