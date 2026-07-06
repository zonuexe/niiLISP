# Ch. 9 — Working with dates and times

niiLISP implements almost none of newLISP's date/time API: only `time`, `time-of-day`, and `sleep` exist as builtins. The calendar/formatting family (`date`, `date-value`, `now`, `date-parse`, `timer`) is entirely unbound.

**Coverage: 3 ✅ / 0 ⚠️ / 6 ❌**  *(updated: `time-of-day` now ms-since-midnight)*

| Feature | Status | Notes |
|---|---|---|
| `time` | ✅ | Returns elapsed milliseconds for evaluating an expression N times, matches book semantics. |
| `sleep` | ✅ | Blocks and returns the argument, consistent with newLISP behavior. |
| `time-of-day` | ✅ | Returns ms since (UTC) midnight, `< 86400000`, matching newLISP's `tv_sec % 86400` (fixed 2026-07-06) |
| `date` | ❌ | Symbol unbound; calling it errors `not a function: nil`. |
| `date-value` | ❌ | Symbol unbound; same error. |
| `now` | ❌ | Symbol unbound; same error. |
| `parse-date` (book's `date-parse`) | ❌ | Symbol unbound; same error. |
| `timer` | ❌ | Symbol unbound; same error. |
| `date-list` | ❌ | Symbol unbound; same error (not a real newLISP function, but was checked per task spec). |

## Divergences & gaps

### ~~`time-of-day` returns epoch ms, not ms-since-midnight~~ — FIXED 2026-07-06

`(time-of-day)` now returns `epoch_ms % 86_400_000` (always `< 86400000`),
matching newLISP's `milliSecTime()` which does `tv_sec % 86400`. Like newLISP,
the reference point is UTC midnight, not local.

### `date` unbound (❌)

Book: `(date)` → `"Mon Mar 19 20:05:02 2006"`; `(date 0)` → `"Wed Dec 31 16:00:00 1969"`; `(date 1136505600 0 "%Y-%m-%d %H:%M:%S")` → `"2006-01-06 00:00:00"`.

```
$ ./target/release/niilisp -e '(println (date))'
niilisp: not a function: nil
$ ./target/release/niilisp -e '(println date)'
nil
```

Confirms `date` is a genuinely unbound symbol, not a builtin returning something book-incompatible.

### `date-value` unbound (❌)

Book: `(date-value 2006 5 11)` → `1147305600`.

```
$ ./target/release/niilisp -e '(println (date-value 2006 5 11))'
niilisp: not a function: nil
```

### `now` unbound (❌)

Book: `(now)` → `(2006 3 19 20 5 2 125475 78 1 0 0)`.

```
$ ./target/release/niilisp -e '(println (now))'
niilisp: not a function: nil
```

### `parse-date` unbound (❌)

Book (as `date-parse`, current newLISP name `parse-date`): `(parse-date "2006-12-13" "%Y-%m-%d")` → `1165968000`.

```
$ ./target/release/niilisp -e '(println (parse-date "2006-12-13" "%Y-%m-%d"))'
niilisp: not a function: nil
```

### `timer` unbound (❌)

Book: `(timer teas-brewed (* 3 60))` schedules an event after N seconds.

```
$ ./target/release/niilisp -e '(println (timer (lambda () (println "fired")) 1))'
niilisp: not a function: nil
```

### `date-list` unbound (❌)

Mentioned in the task's probe list; not actually part of this WikiBook chapter's documented API (the real function is `now`), but confirmed absent regardless:

```
$ ./target/release/niilisp -e '(println (date-list))'
niilisp: not a function: nil
```
