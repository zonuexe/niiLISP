//! Integration tests that run vendored newLISP qa scripts through the built
//! interpreter and check their self-reported result (ADR-0009).
//!
//! These scripts call `(exit)`, so they must run as a subprocess rather than
//! in-process.

use std::process::Command;

fn run_qa(name: &str) -> String {
    run_qa_args(name, &[])
}

/// Run a vendored qa script with extra command-line arguments (available to the
/// script via `(main-args)`), returning its stdout.
fn run_qa_args(name: &str, extra: &[&str]) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let path = format!("references/newlisp/qa-specific-tests/{}", name);
    let output = Command::new(bin)
        .arg(&path)
        .args(extra)
        .output()
        .unwrap_or_else(|e| panic!("failed to launch {}: {}", bin, e));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn qa_exception_passes() {
    let stdout = run_qa("qa-exception");
    assert!(
        stdout.contains("SUCCESSFUL"),
        "qa-exception did not report success:\n{}",
        stdout
    );
}

/// The FFI memory-API slice (ADR-0021): NULL `char*` through `unpack` and
/// `get-string` must error rather than segfault. Needs `struct`/`pack`/`unpack`/
/// `get-string`, which are Unix-only and gated on the `ffi` feature.
#[test]
#[cfg(all(feature = "ffi", unix))]
fn qa_nullstring_passes() {
    let stdout = run_qa("qa-nullstring");
    assert!(
        stdout.contains("SUCCESS unpacking NULL ptr"),
        "qa-nullstring unpack did not report success:\n{}",
        stdout
    );
    assert!(
        stdout.contains("SUCCESS get-string on NULL ptr"),
        "qa-nullstring get-string did not report success:\n{}",
        stdout
    );
}

/// bigint (ADR-0022) plus the helper functions it uses (`seed`/`rand`/`random`/
/// `amb`/`until`/`extend`/`explode`/`main-args`). The script fuzzes the bigint
/// operators over `N` random cases; `N` defaults to 100000 but reads a smaller
/// value from `(main-args -1)`, so a passed argument keeps the test quick while
/// still exercising every operator.
#[cfg(feature = "bigint")]
#[test]
fn qa_bigint_passes() {
    let stdout = run_qa_args("qa-bigint", &["300"]);
    assert!(
        stdout.contains("big ints tested SUCCESSFUL"),
        "qa-bigint did not report success:\n{}",
        stdout
    );
}

/// Factor the first N Fibonacci numbers, using an array-based sieve (ADR-0023)
/// and bigint. Passes, but its `collect-primes` sieves to 1,000,000, which is
/// ~1s release / ~10s debug even at N=12 — too slow for the default run, so it
/// is `#[ignore]`d (run with `cargo test -- --ignored`). Copy-on-write
/// (ADR-0024) is what makes the sieve feasible at all.
#[cfg(feature = "bigint")]
#[test]
#[ignore = "1,000,000-element sieve is ~10s in debug; run explicitly"]
fn qa_factorfibo_passes() {
    let stdout = run_qa_args("qa-factorfibo", &["12"]);
    assert!(
        stdout.contains("Total duration:") && !stdout.contains("ERROR"),
        "qa-factorfibo did not finish cleanly:\n{}",
        stdout
    );
    // Spot-check a known factorization: fibo 11 = 144 = 2^4 * 3^2.
    assert!(
        stdout.contains("144 -> (2 2 2 2 3 3)"),
        "qa-factorfibo factorization looks wrong:\n{}",
        stdout
    );
}

/// A 1000-digit literal squared and divided back, with a per-digit checksum —
/// bigint parsing/printing plus `explode`/`chop`.
#[cfg(feature = "bigint")]
#[test]
fn qa_longnum_passes() {
    let stdout = run_qa("qa-longnum");
    assert!(
        stdout.contains("parsing big integers SUCCESSFUL"),
        "qa-longnum did not report success:\n{}",
        stdout
    );
}

/// The multilingual UTF-8 display test: builds strings in a context `L`, then
/// `(dotree (l L) (println (term l) ":" (eval l)))`. Exercises context switching
/// (ADR-0026), `dotree`/`term`, and character-oriented string handling (ADR-0025).
#[test]
fn qa_utf8_passes() {
    let stdout = run_qa("qa-utf8");
    assert!(
        stdout.contains("Tested UTF-8 font and display performance"),
        "qa-utf8 did not finish:\n{}",
        stdout
    );
    // A round-tripped multi-byte string is displayed, not mangled.
    assert!(
        stdout.contains("Japanese:"),
        "qa-utf8 did not render the context's entries:\n{}",
        stdout
    );
}

/// UTF-8 regex (ADR-0028): literal-character `regex` matches with byte offsets,
/// and character-number round-trips.
#[cfg(feature = "regex")]
#[test]
fn qa_utf8_char_regex_passes() {
    let stdout = run_qa("qa-utf8-char-regex");
    assert!(
        stdout.contains("utf8-char-tests SUCESSFUL")
            && stdout.contains("utf8-regex-tests SUCESSFUL"),
        "qa-utf8-char-regex did not pass:\n{}",
        stdout
    );
}

/// `regex-comp` of a UTF-8 pattern, plus Unicode `upper-case`/`lower-case` on
/// Cyrillic (ADR-0028).
#[cfg(feature = "regex")]
#[test]
fn qa_utf8_special_passes() {
    let stdout = run_qa("qa-utf8-special");
    assert!(
        stdout.contains("UTF-8 compile sucessfull") && !stdout.contains("problem"),
        "qa-utf8-special regex-comp failed:\n{}",
        stdout
    );
    // Cyrillic case folding round-trips.
    assert!(
        stdout.contains("АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ"),
        "qa-utf8-special upper-case failed:\n{}",
        stdout
    );
}

#[cfg(feature = "regex")]
#[test]
fn qa_utf8_compile_passes() {
    let stdout = run_qa("qa-utf8-compile");
    assert!(
        stdout.contains("UTF-8 compile SUCESSFULL"),
        "qa-utf8-compile did not pass:\n{}",
        stdout
    );
}

/// Character-based `dostring` (ADR-0025): the loop variable binds each UTF-8
/// character's code point, not each byte, so a multi-byte string round-trips
/// through `(char c)` and reports its character length via `utf8len`. The script
/// also exercises `unpack`, so it needs the FFI memory API (ADR-0021); the
/// `dostring` behaviour itself is covered feature-independently by a unit test.
#[cfg(feature = "ffi")]
#[test]
fn qa_utf8_ext_passes() {
    let stdout = run_qa("qa-utf8-ext");
    // dostring reconstructs the original string from per-character code points.
    assert!(
        stdout.contains("dostring: 我能吞下玻璃而不伤身体。"),
        "qa-utf8-ext dostring did not iterate characters:\n{}",
        stdout
    );
    // The `unicode:` line prints code points, not raw bytes.
    assert!(
        stdout.contains("unicode: 25105 33021 21534"),
        "qa-utf8-ext did not bind code points:\n{}",
        stdout
    );
    // Byte length 36, character length 12 (utf8len is present).
    assert!(
        stdout.contains("length raw, utf8: 36, 12"),
        "qa-utf8-ext length/utf8len wrong:\n{}",
        stdout
    );
    // explode yields whole characters.
    assert!(
        stdout.contains("(\"我\" \"能\" \"吞\" \"下\""),
        "qa-utf8-ext explode did not keep characters whole:\n{}",
        stdout
    );
}

/// Dictionary API (ADR-0030) plus persistence: populate a namespace from an
/// association list, verify every read, `save` it, `delete` it, `load` it back,
/// and confirm the two saves are byte-identical. Exercises the nil-functor hash
/// dispatch, `save`/`load`/`source` (file I/O slice 2), and `randomize`/
/// `sys-info`. Runs in a temp dir because the oracle writes `Lex.lsp`.
#[test]
fn qa_dictionary_passes() {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let script = std::fs::canonicalize("references/newlisp/qa-specific-tests/qa-dictionary")
        .expect("locate qa-dictionary");
    let dir = std::env::temp_dir().join(format!("niilisp-qa-dict-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let output = Command::new(bin)
        .arg(&script)
        .arg("300") // a small N keeps the benchmark quick
        .current_dir(&dir)
        .output()
        .expect("launch niilisp");
    let _ = std::fs::remove_dir_all(&dir);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Dictionary API tested SUCCESSFUL"),
        "qa-dictionary did not pass:\n{}",
        stdout
    );
}

/// The Cilk API (ADR-0032): `spawn`/`sync`/`abort` fork the interpreter, run the
/// recursive `(fibo 5)` = 8 across forked processes, and collect results via the
/// `sync` inlet callback. Needs real Unix `fork()`, so it is `mt`-gated.
#[cfg(all(feature = "mt", unix))]
#[test]
fn qa_cilk_passes() {
    let stdout = run_qa("qa-cilk");
    assert!(
        stdout.contains("The Cilk API tested SUCCESSFUL"),
        "qa-cilk did not pass:\n{}",
        stdout
    );
}

/// `share` (ADR-0032): a parent writes a packed buffer into an `mmap`ed
/// `MAP_SHARED` page, a `spawn`ed child reads and `unpack`s it, and `sync`
/// returns the result — exercising shared memory across `fork`. Needs `mt`
/// (fork/mmap) and `ffi` (`pack`/`unpack`).
#[cfg(all(feature = "mt", feature = "ffi", unix))]
#[test]
fn qa_share_passes() {
    let stdout = run_qa("qa-share");
    assert!(
        stdout.contains("test passed SUCCESSFUL"),
        "qa-share did not pass:\n{}",
        stdout
    );
}

/// Raw `fork`/`pipe`/`wait-pid` (ADR-0032): two forked processes communicate
/// over a pipe — a counter writes lines, an observer reads them. `mt`-gated.
#[cfg(all(feature = "mt", unix))]
#[test]
fn qa_pipefork_passes() {
    let stdout = run_qa("qa-pipefork");
    assert!(
        stdout.contains("ms per write->read pipe/fork") && !stdout.contains("error"),
        "qa-pipefork did not finish cleanly:\n{}",
        stdout
    );
}

/// The message API (ADR-0032): `send`/`receive` over per-child datagram
/// socketpairs — a status phase (children stream numbers to the parent), a
/// round-trip phase, and a proxy phase where the parent `eval`s received code
/// to relay A→B. `mt`-gated. (qa-msgbig additionally needs `base64-enc` and
/// stream framing for its 80 KB messages, so it is not wired.)
#[cfg(all(feature = "mt", unix))]
#[test]
fn qa_message_passes() {
    let stdout = run_qa("qa-message");
    assert!(
        stdout.contains("Message API tested SUCCESSFUL"),
        "qa-message did not pass:\n{}",
        stdout
    );
}

/// `signal` (ADR-0032): install handlers for SIGUSR1/SIGUSR2, then `exec kill`
/// to deliver them to self; each must fire the Lisp handler. The async C handler
/// sets an atomic flag that the evaluator polls at safe points. `mt`-gated; slow
/// (it sleeps between signals).
#[cfg(all(feature = "mt", unix))]
#[test]
fn qa_siguser_passes() {
    let stdout = run_qa("qa-siguser");
    assert!(
        stdout.contains("signal 30 was fired") && stdout.contains("signal 31 was fired"),
        "qa-siguser handlers did not fire:\n{}",
        stdout
    );
}

/// Stream sockets (ADR-0033): a `fork`ed listener `net-listen`/`net-accept`s a
/// Unix-domain socket, and the parent `net-connect`/`net-send`/`net-select`/
/// `net-receive`s a round trip — the connect/listen/accept/send/receive/select
/// surface the GUI needs. Needs `net` (sockets) and `mt` (fork).
#[cfg(all(feature = "net", feature = "mt", unix))]
#[test]
fn qa_local_domain_passes() {
    let stdout = run_qa("qa-local-domain");
    assert!(
        stdout.contains("UNIX local domain sockets SUCCESSFUL"),
        "qa-local-domain did not pass:\n{}",
        stdout
    );
}

#[test]
fn qa_foop_passes() {
    let stdout = run_qa("qa-foop");
    assert!(
        stdout.contains("FOOP nested 'self' tested SUCCCESSFUL"),
        "qa-foop nested-self did not pass:\n{}",
        stdout
    );
    assert!(
        stdout.contains("FOOP symbol protection SUCCESSFUL"),
        "qa-foop symbol protection did not pass:\n{}",
        stdout
    );
}
