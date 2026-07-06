# Ch. 9 — Working with dates and times

niiLISP implements almost none of newLISP's date/time API: only `time`, `time-of-day`, and `sleep` exist as builtins, and `time-of-day` itself returns the wrong value (raw epoch milliseconds instead of milliseconds since midnight).

**Coverage: 2 ✅ / 1 ⚠️ / 6 ❌**

| Feature | Status | Notes |
|---|---|---|
| `time` | ✅ | Returns elapsed milliseconds for evaluating an expression N times, matches book semantics. |
| `sleep` | ✅ | Blocks and returns the argument, consistent with newLISP behavior. |
| `time-of-day` | ⚠️ | Builtin exists but returns raw epoch milliseconds, not "milliseconds since start of today." |
| `date` | ❌ | Symbol unbound; calling it errors `not a function: nil`. |
| `date-value` | ❌ | Symbol unbound; same error. |
| `now` | ❌ | Symbol unbound; same error. |
| `parse-date` (book's `date-parse`) | ❌ | Symbol unbound; same error. |
| `timer` | ❌ | Symbol unbound; same error. |
| `date-list` | ❌ | Symbol unbound; same error (not a real newLISP function, but was checked per task spec). |

## Divergences & gaps

### `time-of-day` returns epoch ms, not ms-since-midnight (⚠️)

Book: `(time-of-day)` → "return milliseconds since the start of today till now", so the value should always be `< 86400000`.

```
$ ./target/release/niilisp -e '(println (time-of-day))'
1783338857120
```

That's clearly raw Unix epoch time in milliseconds (equivalent to `date +%s%3N`), not time-of-day-relative. Any script porting the book's idiom `(div (- (time-of-day) start-time) 1000)` to compute elapsed seconds still happens to work (since it's a delta), but direct use of the absolute value (e.g. formatting "seconds since midnight") would be wrong.

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
