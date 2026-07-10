# Ch. 15 — More examples

The chapter's complete example programs: **both file-tree text editors and both "On your own terms" override examples now run end-to-end** (after the regex-`replace`, `$idx`, and `global` fixes). The only remaining failure is the countdown timer (still needs `date-value`/`date`/`ostype` — `letn` is now implemented); the AppleScript bridge is macOS-app-specific.

**Coverage: 4 ✅ / 0 ⚠️ / 2 ❌**  *(updated 2026-07-06: `global` unblocks both "On your own terms" programs)*

| Example program | Status | Notes (blocking function if failed) |
|---|---|---|
| On your own terms — `set` alias via `(global 'set!)` | ✅ | Fixed 2026-07-06: `global` implemented; `(constant (global 'set!) set)` installs the alias and `(set! 'q 5)` works |
| On your own terms — custom `println` counter override | ✅ | Fixed 2026-07-06: `(set (global 'println) Output)` overrides the builtin and later `println` calls dispatch to the override |
| Simple countdown timer (`countdown` script) | ❌ | Still missing builtins: `date-value`, `date`, `ostype` unbound (`letn` and `$idx` now implemented) |
| Editing text files in folders (basic, non-recursive) | ✅ | Reads each file, regex-`replace`s, writes back — verified end-to-end (fixed 2026-07-06: regex `replace` now substitutes and re-evaluates per match) |
| Editing text files in a hierarchy (recursive version) | ✅ | Recurses via `directory`/`directory?` and edits each file; the earlier `page`-variable "corruption" was a symptom of the broken `replace`, now resolved (fixed 2026-07-06) |
| Talking to other applications (Illustrator AppleScript circle script) | ❌ (not runnable here) | Platform-specific: requires `osascript`/Adobe Illustrator on macOS; not functionally probed, but relies on the same `exec`/`format`/`set` idioms that work fine standalone — no niiLISP-specific blocker identified beyond the missing app |

## Divergences & gaps

### 1. ~~`global` is unbound~~ — FIXED 2026-07-06

`global`/`global?` are implemented. `(global 'sym…)` declares MAIN symbols global and returns the last, so the "On your own terms" alias/override idiom runs:

```
$ niilisp -e "(constant (global 'set!) set) (set! 'q 5) (println q)"
5
```
(The book's `set!` aliases `set`, not `setf` — `setf` takes a place, not a quoted symbol.)

### 2. `date-value`, `date`, `letn`, `ostype` unbound — breaks the countdown script

```
$ niilisp -e '(println (date-value))'
niilisp: not a function: nil
$ niilisp -e '(println (date (date-value) 0 "%Y-%m-%d %H:%M:%S"))'
niilisp: not a function: nil
$ niilisp -e '(println (letn ((a 1) (b (+ a 1))) b))'
niilisp: not a function: nil
$ niilisp -e '(println ostype)'
nil
```
Grepping the niiLISP source (`src/*.rs`) for `"date-value"`, `"letn"`, `"ostype"` returns zero matches — these are not implemented at all. The countdown script's `set-duration`, `seconds->dhms`, `notify`, and main loop all depend on at least one of these, so the whole program is non-functional.

### 3. `$idx` (implicit dolist index) does not update

```
$ niilisp -e '(dolist (e (quote (a b c))) (println $idx))'
nil
nil
nil
```
newLISP sets `$idx` to the current 0-based iteration index inside `dolist`/`dotimes`. niiLISP leaves it unbound (`nil`) throughout. This breaks `set-duration` in the countdown script, which indexes `'(1 60 3600 86400)` by `$idx` to convert `d:h:m:s` components to seconds.

### 4. Regex-mode `replace` does not substitute (both file-editing examples)

```
$ niilisp -e '(set (quote page) "<tag>old</tag> hi") (replace "<tag>(.*?)</tag>" page "<tag>NEW</tag>" 0) (println page)'
<tag>old</tag> hi
```
Expected (newLISP): `<tag>NEW</tag> hi`. Plain literal-string `replace` (3-arg, no trailing option) works correctly:
```
$ niilisp -e '(set (quote page) "hello world") (replace "world" page "there" 0) (println page)'
hello there
```
but the 4-arg regex form (trailing `0` = regex-match flag) leaves `page` untouched when the pattern contains regex metacharacters/capture groups — even though the standalone `regex` builtin correctly parses the same pattern:
```
$ niilisp -e '(println (regex "<tag>(.*?)</tag>" "<tag>old</tag> hi"))'
("<tag>old</tag>" 0 14 "old" 5 3)
```
So `regex` itself works, but `replace`'s regex-mode substitution path does not use it / does not substitute. This silently no-ops rather than erroring, which is exactly the "unbound symbol returns nil" trap the harness warns about — the basic file-editing example ran to completion, printed "processing file ...", and exited 0, but the target files were never actually modified:
```
$ cat testdir/a.txt   # before and after — identical
<last-edited>old-date</last-edited> hello world
```

### 5. Recursive file-editing example: secondary data corruption

Beyond the regex-`replace` bug above, the recursive tree-walk version additionally corrupts the `page` variable before `write-file`:
```
$ niilisp testdir2-script.lsp
processing file 2026-07-06
niilisp: write-file: expected a string
```
The `println "processing file " pn` line prints the *replacement date string* ("2026-07-06") instead of the filename (`pn`), indicating a variable/argument mixup during the `replace-string-in-file` call chain (likely from the same broken regex-`replace` call misbinding one of its arguments). Not isolated further given time constraints, but it means the recursive example is doubly broken, not just a no-op like the basic version.

### 6. Not independently blocking, but noted

- `div` always performs float division in niiLISP (`(div 10 3)` → `3.3333333333333335`) whereas `/` truncates to an integer for two integer operands (`(/ 10 3)` → `3`). The countdown script's `seconds->dhms` uses `(div s 60)` etc. expecting integer/floor semantics; this would produce incorrect (fractional) day/hour/minute/second breakdowns even if `date-value`/`date` existed. Confirmed but not the primary blocker since the script fails earlier on missing builtins.
- Context-qualified function definitions (`Output:Output`), `map`, `args`, `directory`, `directory?`, `read-file`, `write-file`, `real-path`, `append`, `string`, `starts-with`, `parse`, `format`, `exec`, `sleep`, `mod` all work correctly and are not blockers.
