//! File I/O and filesystem operations (ADR-0029), always compiled in.
//!
//! Handles are opaque integers into an interpreter-side registry, with 0/1/2
//! reserved for stdin/stdout/stderr. Operational failures return `nil`; only
//! type misuse raises an error. Paths are byte-buffer strings converted
//! OS-natively (binary-safe on Unix), consistent with the string model.

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::eval::{Interp, Signal};
use crate::value::Value;

/// The open-file registry (ADR-0029). Indices 0/1/2 are reserved for the
/// standard streams and never handed out; other handles are slots reused from a
/// freelist after `close`.
pub struct FileTable {
    slots: Vec<Option<File>>,
    free: Vec<usize>,
}

impl FileTable {
    pub fn new() -> Self {
        // Occupy 0/1/2 so real handles start at 3 (stdin/stdout/stderr).
        FileTable {
            slots: vec![None, None, None],
            free: Vec::new(),
        }
    }

    fn insert(&mut self, f: File) -> i64 {
        if let Some(i) = self.free.pop() {
            self.slots[i] = Some(f);
            i as i64
        } else {
            self.slots.push(Some(f));
            (self.slots.len() - 1) as i64
        }
    }

    /// Free a handle; `true` if it named an open file (numbers < 3 and unknown
    /// handles are not closable and yield `false`).
    fn close(&mut self, h: i64) -> bool {
        if h >= 3 {
            if let Some(slot) = self.slots.get_mut(h as usize) {
                if slot.is_some() {
                    *slot = None;
                    self.free.push(h as usize);
                    return true;
                }
            }
        }
        false
    }

    fn get_mut(&mut self, h: i64) -> Option<&mut File> {
        if h < 3 {
            return None;
        }
        self.slots.get_mut(h as usize).and_then(|s| s.as_mut())
    }

    /// Register an already-open raw fd (a `pipe` end, ADR-0032, or a socket,
    /// ADR-0033) as a handle, so `read-line`/`write-line`/`close` work on it
    /// uniformly.
    #[cfg(all(unix, any(feature = "mt", feature = "net")))]
    pub fn insert_fd(&mut self, fd: std::os::unix::io::RawFd) -> i64 {
        use std::os::unix::io::FromRawFd;
        self.insert(unsafe { File::from_raw_fd(fd) })
    }

    /// The raw fd backing a handle (for socket `libc` calls, ADR-0033).
    #[cfg(all(unix, feature = "net"))]
    pub fn raw_fd(&self, h: i64) -> Option<std::os::unix::io::RawFd> {
        use std::os::unix::io::AsRawFd;
        if h < 3 {
            return None;
        }
        self.slots
            .get(h as usize)
            .and_then(|s| s.as_ref())
            .map(|f| f.as_raw_fd())
    }
}

/// Write all of `data` to handle `h`, returning the byte count (or `None`). A
/// pub entry point for `net-send` (ADR-0033).
#[cfg(all(unix, feature = "net"))]
pub fn write_handle(interp: &Interp, h: i64, data: &[u8]) -> Option<i64> {
    if write_bytes(interp, h, data) {
        Some(data.len() as i64)
    } else {
        None
    }
}

/// Close handle `h` (for `net-close`, ADR-0033); `true` if it was open.
#[cfg(all(unix, feature = "net"))]
pub fn close_handle(interp: &Interp, h: i64) -> bool {
    interp.files().borrow_mut().close(h)
}

impl Default for FileTable {
    fn default() -> Self {
        Self::new()
    }
}

// ---- argument helpers ----------------------------------------------------

fn int_arg(v: Option<&Value>, what: &str) -> Result<i64, Signal> {
    match v {
        Some(Value::Int(n)) => Ok(*n),
        Some(Value::Float(f)) => Ok(*f as i64),
        _ => Err(Signal::error(format!("{}: expected an integer", what))),
    }
}

fn str_arg<'a>(v: Option<&'a Value>, what: &str) -> Result<&'a [u8], Signal> {
    match v {
        Some(Value::Str(b)) => Ok(b),
        _ => Err(Signal::error(format!("{}: expected a string", what))),
    }
}

#[cfg(unix)]
fn os_from_bytes(bytes: &[u8]) -> std::ffi::OsString {
    use std::os::unix::ffi::OsStrExt;
    OsStr::from_bytes(bytes).to_os_string()
}

#[cfg(not(unix))]
fn os_from_bytes(bytes: &[u8]) -> std::ffi::OsString {
    // Windows: faithful UTF-16 path handling is deferred (ADR-0029); interpret
    // the bytes as (lossy) UTF-8 for now.
    std::ffi::OsString::from(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(unix)]
fn os_to_bytes(os: &OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    os.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn os_to_bytes(os: &OsStr) -> Vec<u8> {
    os.to_string_lossy().into_owned().into_bytes()
}

fn path_arg(v: Option<&Value>, what: &str) -> Result<PathBuf, Signal> {
    Ok(PathBuf::from(os_from_bytes(str_arg(v, what)?)))
}

// ---- handles: open / close / seek ----------------------------------------

fn b_open(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "open")?;
    let mode = str_arg(args.get(1), "open")?;
    let mut oo = OpenOptions::new();
    match mode {
        b"read" => {
            oo.read(true);
        }
        b"write" => {
            oo.write(true).create(true).truncate(true);
        }
        b"append" => {
            oo.append(true).create(true);
        }
        b"update" => {
            oo.read(true).write(true);
        }
        _ => return Err(Signal::error("open: mode must be read/write/append/update")),
    }
    match oo.open(&path) {
        Ok(f) => Ok(Value::Int(interp.files().borrow_mut().insert(f))),
        Err(_) => Ok(Value::Nil),
    }
}

fn b_close(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let h = int_arg(args.first(), "close")?;
    Ok(if interp.files().borrow_mut().close(h) {
        Value::True
    } else {
        Value::Nil
    })
}

fn b_seek(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let h = int_arg(args.first(), "seek")?;
    let mut tbl = interp.files().borrow_mut();
    let f = match tbl.get_mut(h) {
        Some(f) => f,
        None => return Ok(Value::Nil),
    };
    let pos = match args.get(1) {
        None => {
            return Ok(f
                .stream_position()
                .map(|p| Value::Int(p as i64))
                .unwrap_or(Value::Nil))
        }
        Some(v) => int_arg(Some(v), "seek")?,
    };
    let r = if pos == -1 {
        f.seek(SeekFrom::End(0))
    } else {
        f.seek(SeekFrom::Start(pos as u64))
    };
    Ok(r.map(|p| Value::Int(p as i64)).unwrap_or(Value::Nil))
}

// ---- buffered reads/writes on handles ------------------------------------

fn b_write_buffer(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let h = int_arg(args.first(), "write-buffer")?;
    let data = str_arg(args.get(1), "write-buffer")?;
    let n = match args.get(2) {
        Some(v) => (int_arg(Some(v), "write-buffer")?.max(0) as usize).min(data.len()),
        None => data.len(),
    };
    let slice = &data[..n];
    Ok(if write_bytes(interp, h, slice) {
        Value::Int(n as i64)
    } else {
        Value::Nil
    })
}

/// Write raw bytes to handle `h` (1/2 = stdout/stderr, else a file handle),
/// returning whether it succeeded.
fn write_bytes(interp: &Interp, h: i64, data: &[u8]) -> bool {
    match h {
        1 => io::stdout().write_all(data).is_ok(),
        2 => io::stderr().write_all(data).is_ok(),
        _ => match interp.files().borrow_mut().get_mut(h) {
            Some(f) => f.write_all(data).is_ok(),
            None => false,
        },
    }
}

/// `(write-line handle [str])` — write `str` (or the last `read-line`) followed
/// by a newline; returns the byte count, or `nil` on failure.
fn b_write_line(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let h = int_arg(args.first(), "write-line")?;
    let mut line = match args.get(1) {
        Some(Value::Str(b)) => b.to_vec(),
        Some(_) => return Err(Signal::error("write-line: expected a string")),
        None => interp.current_line(),
    };
    line.push(b'\n');
    Ok(if write_bytes(interp, h, &line) {
        Value::Int(line.len() as i64)
    } else {
        Value::Nil
    })
}

/// Read up to `size` bytes (or until `wait` appears) from handle `h` (0 =
/// stdin), returning the bytes and their count. Shared by the `read-buffer`
/// special form.
pub fn read_buffer(
    interp: &Interp,
    h: i64,
    size: i64,
    wait: Option<Vec<u8>>,
) -> Result<(Vec<u8>, usize), Signal> {
    let size = size.max(0) as usize;
    if h == 0 {
        let stdin = io::stdin();
        let mut lock = stdin.lock();
        Ok(read_buffer_from(&mut lock, size, wait.as_deref()))
    } else {
        let mut tbl = interp.files().borrow_mut();
        match tbl.get_mut(h) {
            Some(f) => Ok(read_buffer_from(f, size, wait.as_deref())),
            None => Ok((Vec::new(), 0)),
        }
    }
}

fn read_buffer_from<R: Read>(r: &mut R, size: usize, wait: Option<&[u8]>) -> (Vec<u8>, usize) {
    match wait {
        None => {
            // A single read: return whatever one read yields (up to `size`).
            // A fill-loop would block a socket `recv` waiting for more bytes
            // that a peer holding the connection open never sends (ADR-0033);
            // a regular file returns the whole record in one read regardless.
            let mut buf = vec![0u8; size];
            let n = loop {
                match r.read(&mut buf) {
                    Ok(n) => break n,
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break 0,
                }
            };
            buf.truncate(n);
            (buf, n)
        }
        Some(w) => {
            let mut out = Vec::new();
            let mut byte = [0u8; 1];
            while out.len() < size {
                match r.read(&mut byte) {
                    Ok(0) => break,
                    Ok(_) => {
                        out.push(byte[0]);
                        if !w.is_empty() && out.ends_with(w) {
                            break;
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            let n = out.len();
            (out, n)
        }
    }
}

/// Read a line from handle `h` (0 = stdin), or `None` at EOF. The terminator is
/// stripped: `\n` ends a line and a `\r` is skipped (so `\r\n` works); a lone
/// `\r` is not treated as a break — vanishingly rare and it avoids a pushback.
pub fn read_line(interp: &Interp, h: i64) -> Option<Vec<u8>> {
    if h == 0 {
        let stdin = io::stdin();
        let mut lock = stdin.lock();
        read_line_from(&mut lock)
    } else {
        let mut tbl = interp.files().borrow_mut();
        match tbl.get_mut(h) {
            Some(f) => read_line_from(f),
            None => None,
        }
    }
}

fn read_line_from<R: Read>(r: &mut R) -> Option<Vec<u8>> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    let mut got = false;
    loop {
        match r.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                got = true;
                match byte[0] {
                    b'\n' => break,
                    b'\r' => {}
                    c => line.push(c),
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    if got {
        Some(line)
    } else {
        None
    }
}

fn b_current_line(interp: &Interp, _args: &[Value]) -> Result<Value, Signal> {
    Ok(Value::str(interp.current_line()))
}

fn b_read_line(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let h = match args.first() {
        Some(v) => int_arg(Some(v), "read-line")?,
        None => 0,
    };
    match read_line(interp, h) {
        Some(line) => {
            interp.set_current_line(line.clone());
            Ok(Value::str(line))
        }
        None => Ok(Value::Nil),
    }
}

// ---- whole-file convenience ----------------------------------------------

fn b_read_file(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "read-file")?;
    match fs::read(&path) {
        Ok(bytes) => Ok(Value::str(bytes)),
        Err(_) => Ok(Value::Nil),
    }
}

fn write_whole(args: &[Value], append: bool, what: &str) -> Result<Value, Signal> {
    let path = path_arg(args.first(), what)?;
    let data = str_arg(args.get(1), what)?;
    let r = OpenOptions::new()
        .write(true)
        .create(true)
        .append(append)
        .truncate(!append)
        .open(&path)
        .and_then(|mut f| f.write_all(data));
    Ok(if r.is_ok() {
        Value::Int(data.len() as i64)
    } else {
        Value::Nil
    })
}

fn b_write_file(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    write_whole(args, false, "write-file")
}

fn b_append_file(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    write_whole(args, true, "append-file")
}

// ---- filesystem ----------------------------------------------------------

fn b_directory(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = match args.first() {
        Some(v) => path_arg(Some(v), "directory")?,
        None => PathBuf::from("."),
    };
    let rd = match fs::read_dir(&path) {
        Ok(rd) => rd,
        Err(_) => return Ok(Value::Nil),
    };
    // newLISP lists `.` and `..` too (raw readdir); Rust's read_dir omits them.
    let mut names: Vec<Vec<u8>> = vec![b".".to_vec(), b"..".to_vec()];
    for entry in rd.flatten() {
        names.push(os_to_bytes(&entry.file_name()));
    }
    #[cfg(feature = "regex")]
    if let Some(Value::Str(pat)) = args.get(1) {
        let pattern = String::from_utf8_lossy(pat).into_owned();
        let re = interp.compiled_regex(&pattern, 0)?;
        names.retain(|n| re.is_match(n));
    }
    #[cfg(not(feature = "regex"))]
    let _ = interp;
    Ok(Value::list(names.into_iter().map(Value::str).collect()))
}

fn b_real_path(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = match args.first() {
        Some(v) => path_arg(Some(v), "real-path")?,
        None => PathBuf::from("."),
    };
    match fs::canonicalize(&path) {
        Ok(p) => Ok(Value::str(os_to_bytes(p.as_os_str()))),
        Err(_) => Ok(Value::Nil),
    }
}

fn bool_result(ok: bool) -> Value {
    if ok {
        Value::True
    } else {
        Value::Nil
    }
}

fn b_make_dir(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "make-dir")?;
    Ok(bool_result(fs::create_dir(&path).is_ok()))
}

fn b_remove_dir(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "remove-dir")?;
    Ok(bool_result(fs::remove_dir(&path).is_ok()))
}

fn b_change_dir(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "change-dir")?;
    Ok(bool_result(std::env::set_current_dir(&path).is_ok()))
}

fn b_rename_file(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let from = path_arg(args.first(), "rename-file")?;
    let to = path_arg(args.get(1), "rename-file")?;
    Ok(bool_result(fs::rename(&from, &to).is_ok()))
}

fn b_delete_file(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "delete-file")?;
    Ok(bool_result(fs::remove_file(&path).is_ok()))
}

fn b_file_q(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "file?")?;
    Ok(bool_result(fs::metadata(&path).is_ok()))
}

fn b_directory_q(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "directory?")?;
    Ok(bool_result(
        fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false),
    ))
}

/// `(file-info path [index])` — a fixed 10-element integer list
/// `(size mode device inode links uid gid atime mtime ctime)`; fields a platform
/// lacks are 0. With `index`, returns just that element.
fn b_file_info(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "file-info")?;
    let md = match fs::metadata(&path) {
        Ok(md) => md,
        Err(_) => return Ok(Value::Nil),
    };
    let fields = file_info_fields(&md);
    match args.get(1) {
        Some(v) => {
            let idx = int_arg(Some(v), "file-info")?;
            Ok(usize::try_from(idx)
                .ok()
                .and_then(|k| fields.get(k))
                .map(|n| Value::Int(*n))
                .unwrap_or(Value::Nil))
        }
        None => Ok(Value::list(fields.iter().map(|n| Value::Int(*n)).collect())),
    }
}

#[cfg(unix)]
fn file_info_fields(md: &fs::Metadata) -> [i64; 10] {
    use std::os::unix::fs::MetadataExt;
    [
        md.size() as i64,
        md.mode() as i64,
        md.dev() as i64,
        md.ino() as i64,
        md.nlink() as i64,
        md.uid() as i64,
        md.gid() as i64,
        md.atime(),
        md.mtime(),
        md.ctime(),
    ]
}

#[cfg(not(unix))]
fn file_info_fields(md: &fs::Metadata) -> [i64; 10] {
    use std::time::UNIX_EPOCH;
    let secs = |t: std::io::Result<std::time::SystemTime>| -> i64 {
        t.ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    };
    [
        md.len() as i64,
        0,
        0,
        0,
        0,
        0,
        0,
        secs(md.accessed()),
        secs(md.modified()),
        secs(md.created()),
    ]
}

// ---- environment ---------------------------------------------------------

fn b_env(_i: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let name = String::from_utf8_lossy(str_arg(args.first(), "env")?).into_owned();
    match args.get(1) {
        None => match std::env::var(&name) {
            Ok(val) => Ok(Value::str(val.into_bytes())),
            Err(_) => Ok(Value::Nil),
        },
        Some(Value::Nil) => {
            std::env::remove_var(&name);
            Ok(Value::True)
        }
        Some(Value::Str(v)) => {
            std::env::set_var(&name, OsStr::new(&String::from_utf8_lossy(v).into_owned()));
            Ok(Value::True)
        }
        Some(_) => Err(Signal::error("env: value must be a string or nil")),
    }
}

// ---- source serialisation: source / save / load (ADR-0030) ---------------

fn build_source(interp: &Interp, syms: &[Value]) -> Result<String, Signal> {
    let mut out = String::new();
    for s in syms {
        match s {
            Value::Symbol(id) | Value::Context(id) => out.push_str(&interp.source_of(*id)),
            _ => return Err(Signal::error("save/source: expected symbols")),
        }
    }
    Ok(out)
}

fn b_source(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    Ok(Value::str(build_source(interp, args)?.into_bytes()))
}

fn b_save(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "save")?;
    // (save file) with no symbols dumps the whole workspace, like newLISP;
    // (save file sym...) dumps just the named symbols.
    let src = if args.len() > 1 {
        build_source(interp, &args[1..])?
    } else {
        build_workspace_source(interp)
    };
    Ok(bool_result(fs::write(&path, src).is_ok()))
}

/// Serialise every user-defined MAIN-level symbol (and context) to loadable
/// source, for the no-symbol `(save file)` form. Excludes built-in primitives,
/// unset (`nil`) symbols, and `$`-prefixed system symbols, matching newLISP.
fn build_workspace_source(interp: &Interp) -> String {
    let mut ids = interp.main_symbol_ids();
    ids.sort_by_key(|id| interp.sym_name(*id));
    let mut out = String::new();
    for id in ids {
        if interp.sym_name(id).starts_with('$') {
            continue;
        }
        match interp.lookup(id) {
            Value::Nil | Value::Builtin(_) => continue,
            _ => out.push_str(&interp.source_of(id)),
        }
    }
    out
}

fn b_load(interp: &Interp, args: &[Value]) -> Result<Value, Signal> {
    let path = path_arg(args.first(), "load")?;
    match fs::read(&path) {
        Ok(bytes) => interp.read_and_eval(&bytes),
        Err(_) => Ok(Value::Nil),
    }
}

pub fn install(interp: &Interp) {
    interp.register_builtin("open", b_open);
    interp.register_builtin("close", b_close);
    interp.register_builtin("seek", b_seek);
    interp.register_builtin("write-buffer", b_write_buffer);
    interp.register_builtin("read-line", b_read_line);
    interp.register_builtin("write-line", b_write_line);
    interp.register_builtin("current-line", b_current_line);
    interp.register_builtin("read-file", b_read_file);
    interp.register_builtin("write-file", b_write_file);
    interp.register_builtin("append-file", b_append_file);
    interp.register_builtin("directory", b_directory);
    interp.register_builtin("real-path", b_real_path);
    interp.register_builtin("make-dir", b_make_dir);
    interp.register_builtin("remove-dir", b_remove_dir);
    interp.register_builtin("change-dir", b_change_dir);
    interp.register_builtin("rename-file", b_rename_file);
    interp.register_builtin("delete-file", b_delete_file);
    interp.register_builtin("file?", b_file_q);
    interp.register_builtin("directory?", b_directory_q);
    interp.register_builtin("file-info", b_file_info);
    interp.register_builtin("env", b_env);
    interp.register_builtin("source", b_source);
    interp.register_builtin("save", b_save);
    interp.register_builtin("load", b_load);
    // `read-buffer` is a place-taking special form (see `sf_read_buffer`).
}
