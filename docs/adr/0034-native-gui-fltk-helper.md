# A native GUI: an fltk helper process driven over a socket (gs:-inspired)

niiLISP reaches newLISP-GS's *language* substrate (file I/O, `process`, sockets,
`eval-string`), but `guiserver.lsp` / `guiserver.jar` are not vendored and depend
on a JVM. Rather than ship the Java server, build a **native Rust GUI helper**
whose API is reverse-engineered from the `gs:` vocabulary. Design was grilled
before writing.

## Compatibility stance: gs:-inspired, not bug-compatible

- **Chosen:** borrow the `gs:` widget/layout/event **vocabulary** as a design
  reference and keep function names/argument shapes close to the original, so
  existing `gs:` scripts run **with as little rewriting as possible** — but with
  **no behaviour guarantee** (a different toolkit won't match Swing's pixels,
  fonts, or layout arithmetic).
- **Rejected:** bug-for-bug compatibility with the Java guiserver. Reproducing
  Swing's exact layout/event/pixel behaviour on another toolkit is near-impossible
  and low-value; it would yield a "looks compatible but isn't" trap.
- **Consequence, stated plainly:** the GUI does **not** advance the project's
  overriding "run existing newLISP assets" goal in a guaranteed way — it provides
  a `gs:`-shaped API, not a certified runtime for legacy GUI scripts.

## Architecture: a separate helper process, driven over a socket

- **Chosen:** a standalone Rust binary (`niilisp-gui`) owns **its own main
  thread** and runs the toolkit's event loop; niiLISP drives it over a socket.
  This is why newLISP is two processes: a GUI toolkit's event loop wants the main
  thread (macOS's Cocoa *requires* the UI on the main thread), which fights a
  single-threaded tree-walker. A process boundary sidesteps it, a GUI crash does
  not take down the interpreter, and **the substrate already exists** (`net-*` +
  `eval-string`, ADR-0031/0033) so the integration cost is near zero.
- **Rejected:** an in-process GUI thread (breaks on main-thread-only toolkits) or
  giving the GUI the main thread and inverting control into the interpreter (worst
  fit for a single-threaded evaluator).

## Toolkit: fltk-rs

- **Chosen:** `fltk-rs` — a **retained-mode, imperative** toolkit (`Button::new(…);
  btn.set_callback(…)`) that maps directly onto `gs:`'s "create a widget by id,
  then mutate it by id" model, is lightweight, builds fast, is cross-platform, and
  needs no GPU stack. Its C++ dependency is confined to the **helper binary**, so
  the interpreter stays pure.
- **Rejected:** `egui`/`iced` (immediate-mode / Elm — an impedance mismatch with
  the retained `gs:` model, and they pull `wgpu`), `gtk-rs` (heavy native dep),
  `slint` (its own DSL).

## Transport and protocol

- **Transport:** one **full-duplex localhost TCP socket**. niiLISP `net-listen`s a
  port, launches the helper (via `process`) with the port, and the helper
  `net-connect`s back; commands flow niiLISP→helper and events helper→niiLISP on
  the one socket.
- **Commands (niiLISP→helper):** line-oriented, space-separated tokens, with
  arbitrary text (labels) **base64**-encoded — e.g. `frame P 100 100 400 300
  <b64-title>`. The helper's parser is split + base64-decode. (Mirrors
  newLISP-GS's actual mechanism, so the Lisp `gs:` module can stay close to the
  original — the "runs with few edits" goal.)
- **Events (helper→niiLISP):** a **niiLISP source line** the event loop
  `eval-string`s — e.g. `(my-click "B")`.

## The `gs:` API and event model

- Widgets are **retained and id-based**; the helper keeps an `id → widget` map.
- A vendored niiLISP **`.lsp` module** defines `gs:frame`/`gs:button`/… as thin
  senders (format + base64 + `net-send`), plus `gs:init` (launch + connect) and
  `gs:listen` / `gs:check-event` (the event loop: `net-receive` + `eval-string`,
  or `net-select` for the non-blocking poll). We write this module ourselves
  (the original is not vendored), keeping names/args close to `gs:`.
- Creating a widget registers a handler name; the helper emits `(handler id …)`
  on the event, which the loop evaluates.

## Scope and acceptance

- **First slice:** `gs:init`, `gs:frame`/`gs:window`, `gs:panel` + one layout,
  `gs:add-to`, `gs:button`/`gs:label`/`gs:text-field`, `gs:set-text`/
  `gs:set-background`, button-click events, `gs:listen`/`gs:check-event`.
  (`gs:canvas` + drawing is a later slice.)
- **Acceptance:** the GUI is **display-dependent**, so end-to-end is not
  CI-automatable (FLTK needs a display; no `gs:` oracle is vendored). So:
  **(1)** unit-test the protocol layer (the `gs:` module produces the right command
  bytes) — display-free, automatable; **(2)** a manual demo script (a window with a
  button and label) verified by a human on a desktop. The real window is not
  auto-verified.

## Packaging: a `gui` feature (default off) and a second binary

- **Chosen:** a `gui` Cargo feature, **default off** (unlike `ffi`/`mt`/`net`):
  the toolkit is heavy, display-dependent, and niche. `gui = ["dep:fltk"]`. The
  helper is a second binary target `src/bin/niilisp-gui.rs` with
  `required-features = ["gui"]`; the optional `fltk` dep is linked only into it,
  never into `niilisp`. The `gs:` `.lsp` module is vendored (e.g. `lib/gui.lsp`);
  `gs:init` finds the helper via an env var (e.g. `NIILISP_GUI`), leaving room to
  honour `NEWLISPDIR`/`guiserver.lsp` naming for legacy scripts.

## Consequences

- A new binary + toolkit dependency, isolated behind `gui`/the helper.
- End-to-end GUI is verified manually; CI covers only the protocol layer.
- The GUI is a `gs:`-shaped API, explicitly not a guaranteed legacy-script runtime.
