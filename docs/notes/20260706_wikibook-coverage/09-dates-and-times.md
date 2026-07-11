# Ch. 9 — Working with dates and times

The calendar/formatting family is implemented under
[ADR-0037](../../adr/0037-dates-and-times.md): `date`, `date-value`, `date-list`,
`now`, and `date-parse`. Only the one-shot `timer` (a `SIGALRM` scheduler) is
still missing.

**Coverage: 8 ✅ / 0 ⚠️ / 1 ❌**  *(updated 2026-07-06: date family implemented)*

| Feature | Status | Notes |
|---|---|---|
| `time` | ✅ | Elapsed milliseconds for evaluating an expression N times. |
| `sleep` | ✅ | Blocks and returns the argument. |
| `time-of-day` | ✅ | ms since (UTC) midnight, `< 86400000` (fixed 2026-07-06). |
| `date-value` | ✅ | `(date-value 2002 2 27 18 21 30)` → `1014834090`; UTC seconds since 1970, list or component args; no-arg → now. |
| `date-list` | ✅ | UTC breakdown `(year month day hour min sec day-of-year day-of-week)`, optional index. |
| `now` | ✅ | 11 local-time integers (…tz-offset-minutes, DST); optional minute-offset and index. |
| `date` | ✅ | Local date/time string via `strftime`; optional seconds, minute-offset, and format; out-of-range → `nil`. |
| `date-parse` | ✅ | `(date-parse str format)` → UTC seconds (via `strptime`); Unix-only, as in newLISP (`nil` on the pure/Windows build). |
| `timer` | ❌ | Not implemented — a `SIGALRM`/`SIGVTALRM`/`SIGPROF` one-shot scheduler, deferred to the signal machinery (ADR-0037). |

## Notes

- `date-value`/`date-list` are pure-Rust and **UTC** (matching newLISP's definition), so they are timezone-independent and available even in the `--no-default-features` build.
- `date`/`now`'s **local** timezone and `strftime`/`strptime` come from `libc` behind the default-on `date` feature; the pure build falls back to UTC formatting (a `strftime` subset), `now`'s timezone/DST fields are `0`, and `date-parse` returns `nil`.

```
$ niilisp -e "(println (date-value 2002 2 27 18 21 30))"
1014834090
$ niilisp -e "(println (date-list 1014834090))"
(2002 2 27 18 21 30 58 3)
$ niilisp -e '(println (date 1014834090 0 "%Y-%m-%d %H:%M:%S"))'   # local time
```
