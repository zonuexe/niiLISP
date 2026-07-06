//! The interactive REPL.
//!
//! Two implementations, selected at compile time by the `readline` feature:
//!
//! * **`readline` on (default):** a `rustyline`-backed loop with line editing,
//!   persistent history, matching-bracket highlighting, multi-line continuation
//!   (an incomplete form keeps editing instead of erroring), and Tab completion
//!   over interned symbols and primitives. Pure Rust, no system libreadline.
//! * **`readline` off (`--no-default-features`):** a dependency-free loop that
//!   reads one line at a time from stdin, preserving the zero-dependency build.
//!
//! Both share the crate-level `read_forms` / `signal_message` helpers so the
//! read/eval/print behaviour stays identical; only the input layer differs.

use crate::eval::Interp;

/// A form is "complete enough to submit" unless the reader stopped mid-token —
/// an unclosed list, string, brace string, tag string, or a bare end-of-input.
/// Those (and only those) mean the user should keep typing on the next line.
#[cfg(feature = "readline")]
fn input_is_incomplete(err: &str) -> bool {
    err == "unexpected end of input" || err.starts_with("unterminated")
}

/// Evaluate every form in `line`, printing each result (or error) — the shared
/// "print" half of the loop, used by both front-ends once a line is in hand.
fn eval_line(interp: &Interp, line: &[u8]) {
    let forms = match crate::read_forms(interp, line) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("read error: {}", e);
            return;
        }
    };
    for form in &forms {
        match interp.eval(form) {
            Ok(v) => println!("{}", interp.repr(&v)),
            Err(sig) => eprintln!("{}", crate::signal_message(interp, sig)),
        }
    }
}

// ---------------------------------------------------------------------------
// rustyline-backed REPL (default)
// ---------------------------------------------------------------------------

#[cfg(feature = "readline")]
mod editing {
    use super::{eval_line, input_is_incomplete};
    use crate::eval::Interp;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use std::borrow::Cow;

    use rustyline::completion::{Completer, Pair};
    use rustyline::error::ReadlineError;
    use rustyline::highlight::{CmdKind, Highlighter, MatchingBracketHighlighter};
    use rustyline::hint::Hinter;
    use rustyline::validate::{ValidationContext, ValidationResult, Validator};
    use rustyline::{Context, Editor, Helper};

    /// Characters that terminate a symbol token when scanning back for the word
    /// under the cursor to complete. Mirrors the reader's delimiters: whitespace,
    /// list/quote punctuation, and the three string-literal openers/closers.
    fn is_delimiter(c: char) -> bool {
        c.is_whitespace() || matches!(c, '(' | ')' | '"' | '\'' | '{' | '}' | '[' | ']')
    }

    /// The byte offset where the symbol under `pos` starts (the completion anchor).
    fn word_start(line: &str, pos: usize) -> usize {
        line[..pos]
            .char_indices()
            .rev()
            .find(|(_, c)| is_delimiter(*c))
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0)
    }

    /// The rustyline helper. The trait impls are hand-written (no `derive`
    /// feature, so the proc-macro/`unicode-ident` chain stays out of the tree):
    /// highlighting delegates to the bracket matcher, hinting is the default
    /// no-op, and completion + validation run against the live interpreter
    /// (`interp` borrows it for the REPL's lifetime).
    struct ReplHelper<'a> {
        highlighter: MatchingBracketHighlighter,
        interp: &'a Interp,
    }

    impl Helper for ReplHelper<'_> {}

    /// No inline suggestions; the default `hint` returns `None`.
    impl Hinter for ReplHelper<'_> {
        type Hint = String;
    }

    /// Delegate matching-bracket highlighting to rustyline's built-in.
    impl Highlighter for ReplHelper<'_> {
        fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
            self.highlighter.highlight(line, pos)
        }
        fn highlight_char(&self, line: &str, pos: usize, kind: CmdKind) -> bool {
            self.highlighter.highlight_char(line, pos, kind)
        }
    }

    impl Completer for ReplHelper<'_> {
        type Candidate = Pair;

        fn complete(
            &self,
            line: &str,
            pos: usize,
            _ctx: &Context<'_>,
        ) -> rustyline::Result<(usize, Vec<Pair>)> {
            let start = word_start(line, pos);
            let prefix = &line[start..pos];
            if prefix.is_empty() {
                return Ok((start, Vec::new()));
            }
            // Interned session symbols plus the primitive names, de-duplicated and
            // name-sorted via the BTreeSet.
            let interner = self.interp.interner.borrow();
            let primitives = self.interp.primitive_names();
            let mut names: BTreeSet<&str> = interner.all_names().collect();
            names.extend(primitives.iter().map(String::as_str));
            let candidates = names
                .into_iter()
                .filter(|n| n.starts_with(prefix))
                .map(|n| Pair {
                    display: n.to_string(),
                    replacement: n.to_string(),
                })
                .collect();
            Ok((start, candidates))
        }
    }

    impl Validator for ReplHelper<'_> {
        fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
            match crate::read_forms(self.interp, ctx.input().as_bytes()) {
                // A clean parse (or a hard syntax error) is submitted; the eval
                // loop reports real errors. Only a truncated form keeps editing.
                Err(msg) if input_is_incomplete(&msg) => Ok(ValidationResult::Incomplete),
                _ => Ok(ValidationResult::Valid(None)),
            }
        }
    }

    /// `~/.niilisp_history`, or `None` when no home directory is discoverable.
    fn history_path() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| {
                let mut p = PathBuf::from(home);
                p.push(".niilisp_history");
                p
            })
    }

    pub fn run(interp: &Interp) {
        eprintln!(
            "niilisp {} - type expressions, Ctrl-D to exit",
            env!("CARGO_PKG_VERSION")
        );

        let mut rl: Editor<ReplHelper, rustyline::history::DefaultHistory> = match Editor::new() {
            Ok(rl) => rl,
            Err(e) => {
                // Terminal has no line-editing support (e.g. a pipe): fall back.
                eprintln!("niilisp: line editing unavailable ({e}); using basic input");
                return super::basic::run(interp);
            }
        };
        rl.set_helper(Some(ReplHelper {
            highlighter: MatchingBracketHighlighter::new(),
            interp,
        }));

        let history = history_path();
        if let Some(path) = &history {
            let _ = rl.load_history(path);
        }

        loop {
            match rl.readline("niilisp> ") {
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let _ = rl.add_history_entry(line.as_str());
                    eval_line(interp, line.as_bytes());
                }
                Err(ReadlineError::Interrupted) => continue, // Ctrl-C: drop the line
                Err(ReadlineError::Eof) => break,            // Ctrl-D: exit
                Err(e) => {
                    eprintln!("input error: {}", e);
                    break;
                }
            }
        }

        if let Some(path) = &history {
            let _ = rl.save_history(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Dependency-free fallback REPL (used when `readline` is off, and when
// rustyline cannot initialise a terminal)
// ---------------------------------------------------------------------------

mod basic {
    use super::eval_line;
    use crate::eval::Interp;

    pub fn run(interp: &Interp) {
        use std::io::{BufRead, Write};
        #[cfg(not(feature = "readline"))]
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
            if line.trim().is_empty() {
                continue;
            }
            eval_line(interp, line.as_bytes());
        }
    }
}

/// Start the interactive REPL, using line editing when the `readline` feature is
/// compiled in and falling back to plain stdin otherwise.
pub fn run(interp: &Interp) {
    #[cfg(feature = "readline")]
    {
        editing::run(interp);
    }
    #[cfg(not(feature = "readline"))]
    {
        basic::run(interp);
    }
}

#[cfg(all(test, feature = "readline"))]
mod tests {
    use super::input_is_incomplete;
    use crate::eval::Interp;

    /// Classify a source the way the REPL validator does: incomplete inputs
    /// keep editing; everything else (valid or a hard syntax error) is submitted.
    fn incomplete(src: &str) -> bool {
        let interp = Interp::new();
        match crate::read_forms(&interp, src.as_bytes()) {
            Err(msg) => input_is_incomplete(&msg),
            Ok(_) => false,
        }
    }

    #[test]
    fn truncated_forms_keep_editing() {
        // Each of the reader's mid-token stops must ask for a continuation line.
        assert!(incomplete("(+ 1 2")); // unterminated list
        assert!(incomplete("(a (b c)")); // still one open paren
        assert!(incomplete("\"abc")); // unterminated string
        assert!(incomplete("{brace")); // unterminated brace string
        assert!(incomplete("[text]body")); // unterminated tag string
    }

    #[test]
    fn complete_or_broken_forms_are_submitted() {
        // Valid input submits...
        assert!(!incomplete("(+ 1 2)"));
        assert!(!incomplete("42"));
        assert!(!incomplete("")); // blank line
                                  // ...and so does a hard syntax error, so the eval loop can report it
                                  // rather than trapping the user in an unclosable continuation.
        assert!(!incomplete(")"));
    }
}
