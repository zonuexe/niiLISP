# Dates and times: a pure-Rust UTC core with a libc local-time refinement

The *Working with dates and times* WikiBook chapter is almost entirely unbuilt:
only `time` and `sleep` exist. This ADR adds `now`, `date`, `date-value`,
`date-list`, and `date-parse`. The hard part is that the Rust standard library
has **no calendar or timezone support at all** — `SystemTime` gives a raw
duration since the epoch and nothing else — while newLISP's `date`/`now` report
**local** wall-clock time using the C library.

The overriding goal (per `CONTEXT.md`) is compatibility with existing newLISP
assets, and newLISP itself is a thin wrapper over C `localtime`/`mktime`/
`strftime`/`strptime`. Where a decision is under-determined we follow the 10.7.x
manual's observable behaviour.

## A pure-Rust UTC core, always on; libc only for local time

- **Chosen:** split the work in two.
  - **UTC core — always compiled, zero-dependency.** `date-value` (date
    components → seconds since 1970-01-01 UTC), `date-list` (seconds → components,
    UTC), and the calendar breakdown behind `now` are implemented with the
    standard civil-date algorithms (`days_from_civil` / `civil_from_days`) over an
    `i64` epoch — no library, correct on every platform. newLISP's own
    `date-value`/`date-list` are **defined as UTC**, so this matches exactly.
  - **Local-time refinement — via `libc`, behind a default-on `date` feature.**
    `date`'s local formatted string (`localtime_r` + `strftime`), `now`'s local
    components and timezone-offset/DST fields, and `date-parse` (`strptime`) use
    `libc`, which the `mt`/`net` features already pull in on Unix. A new
    `date = ["dep:libc"]` feature in the default set makes this explicit and
    self-documenting, mirroring the per-capability feature pattern
    (`ffi`/`mt`/`net`/`regex`).
- **Rejected:** a pure-Rust date crate (`time`/`chrono`). It would give
  cross-platform local time and `strftime` without `libc`, but it is a heavy
  dependency (timezone database, parsing) that the project's zero-dependency
  `--no-default-features` build deliberately avoids, and it would still not be
  *bug-compatible* with newLISP's C-library formatting edge cases.
- **Rejected:** UTC-only everywhere (no `libc`). Simplest and fully portable, but
  `date`/`now` would report UTC where newLISP reports local time — a visible
  divergence for the chapter's worked examples.

## The pure / non-Unix fallback is UTC, documented — not an error

- **Chosen:** with `--no-default-features` (or on a non-`libc` platform), the
  builtins still work: `date` formats in **UTC** using a small built-in subset of
  `strftime` specifiers (`%Y %m %d %H %M %S %j %w %a %b %A %B %p %% ` and the
  default no-format string), `now`'s timezone-offset and DST fields are `0`, and
  `date-parse` returns `nil`. This keeps the pure build fully functional (dates
  are core, not optional like FFI), just without local-timezone adjustment.
- **Rejected:** compiling the date builtins out of the pure build. Dates are used
  by ordinary scripts (the *More examples* countdown), unlike the
  platform/hardware features (`ffi`/`mt`/`net`) that legitimately vanish; a script
  should not get `nil` from `(date-value 2000 1 1)`.

## Scope: `now`/`date`/`date-value`/`date-list`/`date-parse`; `timer` deferred

- **Chosen:** the four value-producing functions plus `date-parse`. `now` returns
  the 11-integer list newLISP documents — year, month, day, hour, minute, second,
  microsecond, day-of-year, day-of-week, timezone-offset-minutes, DST-flag — with
  the optional minute-offset and index arguments.
- **Deferred:** `timer` (a one-shot `SIGALRM`/`SIGVTALRM`/`SIGPROF` countdown that
  calls a handler). It is Unix-only, belongs with the signal/alarm machinery of
  [ADR-0032](0032-cilk-fork-multitasking.md) rather than the calendar code, and no
  near-term coverage target needs it (the *More examples* countdown drives itself
  with `date-value` deltas, not `timer`). `date-parse` stays Unix-only exactly as
  in newLISP (it is `strptime`-based and unavailable on Windows).

## Epoch and representation

- **Chosen:** an `i64` count of seconds since 1970-01-01T00:00:00 UTC is the
  interchange value between all of these (as in newLISP). `date` returns `nil` for
  an out-of-range seconds argument. `now`'s microsecond field comes from the
  sub-second part of the system clock; the pure build reports it too (it needs no
  `libc`).

## Consequences

- Unblocks the *Working with dates and times* chapter and the *More examples*
  countdown timer (which needs `date-value`/`date`).
- Adds one optional dependency edge (`date → libc`) already present transitively
  via `mt`/`net`; the pure build gains no dependency and computes in UTC.
- `timer` and sub-second scheduling remain open; both are bounded Unix-only
  follow-ons that reuse the signal machinery rather than this calendar code.
