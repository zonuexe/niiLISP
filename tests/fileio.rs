//! Hermetic file-I/O tests (ADR-0029). `qa-lfs` itself is interactive (it
//! prompts on stdin) and writes a 5 GB file, so it cannot run in CI; this
//! reproduces its write → seek → read-buffer → verify logic at small scale in a
//! unique temp directory, plus the whole-file and filesystem builtins.

use std::process::Command;

/// Run a niiLISP program (written to a temp `.lsp` file) and return its stdout.
fn run_program(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-fileio-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("prog.lsp");
    // Make the program's paths absolute to this fresh directory.
    let src = src.replace("<DIR>", dir.to_str().unwrap());
    std::fs::write(&script, src).expect("write script");
    let out = Command::new(bin)
        .arg(&script)
        .output()
        .expect("launch niilisp");
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The qa-lfs core: write fixed-length records, then seek to each and read it
/// back with `read-buffer`, verifying every record round-trips.
#[test]
fn write_seek_read_buffer_roundtrip() {
    let src = r##"
(set 'file (open "<DIR>/records" "write"))
(dotimes (i 100)
    (set 'rec (append (format "%08d" i) (dup "#" 12)))
    (write-buffer file rec 20))
(close file)
(set 'file (open "<DIR>/records" "read"))
(set 'ok true)
(for (i 0 99)
    (set 'want (append (format "%08d" i) (dup "#" 12)))
    (seek file (mul i 20))
    (read-buffer file got 20)
    (if (!= want got) (set 'ok nil)))
(close file)
(if ok (println ">>> roundtrip OK") (println ">>> roundtrip FAIL"))
(exit)
"##;
    let out = run_program("roundtrip", src);
    assert!(
        out.contains(">>> roundtrip OK"),
        "unexpected output:\n{}",
        out
    );
}

/// Whole-file read/write/append, `read-line` splitting, and EOF → nil.
#[test]
fn whole_file_and_read_line() {
    let src = r##"
(println "wf=" (write-file "<DIR>/t.txt" "alpha\nbeta\n"))
(println "af=" (append-file "<DIR>/t.txt" "gamma\n"))
(println "rf=[" (read-file "<DIR>/t.txt") "]")
(set 'h (open "<DIR>/t.txt" "read"))
(println "l1=" (read-line h))
(println "l2=" (read-line h))
(println "l3=" (read-line h))
(println "l4=" (read-line h))
(close h)
(exit)
"##;
    let out = run_program("wholefile", src);
    assert!(
        out.contains("wf=11"),
        "write-file byte count wrong:\n{}",
        out
    );
    assert!(
        out.contains("af=6"),
        "append-file byte count wrong:\n{}",
        out
    );
    assert!(
        out.contains("rf=[alpha\nbeta\ngamma\n]"),
        "read-file content wrong:\n{}",
        out
    );
    assert!(out.contains("l1=alpha"), "read-line 1 wrong:\n{}", out);
    assert!(out.contains("l3=gamma"), "read-line 3 wrong:\n{}", out);
    assert!(out.contains("l4=nil"), "read-line at EOF not nil:\n{}", out);
}

/// Filesystem operations: make-dir/directory?/rename-file/delete-file/file?.
#[test]
fn filesystem_operations() {
    let src = r##"
(println "mkdir=" (make-dir "<DIR>/sub"))
(println "isdir=" (directory? "<DIR>/sub"))
(write-file "<DIR>/a.txt" "x")
(println "exists-a=" (file? "<DIR>/a.txt"))
(println "rename=" (rename-file "<DIR>/a.txt" "<DIR>/b.txt"))
(println "gone-a=" (file? "<DIR>/a.txt"))
(println "have-b=" (file? "<DIR>/b.txt"))
(println "del=" (delete-file "<DIR>/b.txt"))
(println "gone-b=" (file? "<DIR>/b.txt"))
(println "open-missing=" (open "<DIR>/none" "read"))
(exit)
"##;
    let out = run_program("fsops", src);
    for expect in [
        "mkdir=true",
        "isdir=true",
        "exists-a=true",
        "rename=true",
        "gone-a=nil",
        "have-b=true",
        "del=true",
        "gone-b=nil",
        "open-missing=nil",
    ] {
        assert!(out.contains(expect), "missing `{}` in:\n{}", expect, out);
    }
}
