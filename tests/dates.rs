//! Dates and times (ADR-0037). Assertions use the timezone-independent UTC core
//! (`date-value`/`date-list` are defined as UTC) so they pass in any timezone;
//! `now`/`date` are exercised for shape only, and `date-parse` tolerates the
//! `date` feature being off (it returns nil there, as on Windows).

use std::process::Command;

fn run(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-date-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("p.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn date_value_is_utc_seconds() {
    // The exact value from the newLISP manual: (date-value (now)) → 1014834090
    // for 2002-02-27 18:21:30 UTC.
    let src = r#"
(println (date-value 2002 2 27 18 21 30))
(println (date-value '(2002 2 27 18 21 30)))
(println (date-value 1970 1 1))
(exit)
"#;
    let out = run("date-value", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(
        l.first().copied(),
        Some("1014834090"),
        "components:\n{}",
        out
    );
    assert_eq!(l.get(1).copied(), Some("1014834090"), "list form:\n{}", out);
    assert_eq!(l.get(2).copied(), Some("0"), "epoch:\n{}", out);
}

#[test]
fn date_list_breaks_down_utc() {
    // year month day hour min sec day-of-year day-of-week (Wed = 3, doy 58).
    let src = r#"
(println (date-list 1014834090))
(println (date-list 1014834090 6))
(println (date-value (date-list 1014834090)))
(exit)
"#;
    let out = run("date-list", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(
        l.first().copied(),
        Some("(2002 2 27 18 21 30 58 3)"),
        "breakdown:\n{}",
        out
    );
    assert_eq!(
        l.get(1).copied(),
        Some("58"),
        "index (day-of-year):\n{}",
        out
    );
    assert_eq!(
        l.get(2).copied(),
        Some("1014834090"),
        "round-trip:\n{}",
        out
    );
}

#[test]
fn now_has_eleven_integers() {
    let src = r#"
(println (length (now)))
(println (integer? (now 0 0)))
(exit)
"#;
    let out = run("now", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(l.first().copied(), Some("11"), "now length:\n{}", out);
    assert_eq!(
        l.get(1).copied(),
        Some("true"),
        "now element type:\n{}",
        out
    );
}

#[test]
fn date_formats_a_string_with_the_year() {
    // Local timezone shifts a mid-year noon by at most ±14h, so the year holds.
    let src = r#"
(println (date (date-value 2000 6 15 12 0 0) 0 "%Y"))
(println (string? (date (date-value))))
(exit)
"#;
    let out = run("date", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(l.first().copied(), Some("2000"), "year format:\n{}", out);
    assert_eq!(
        l.get(1).copied(),
        Some("true"),
        "date is a string:\n{}",
        out
    );
}

#[test]
fn date_parse_round_trips_when_available() {
    // With the `date` feature (Unix) date-parse returns UTC seconds; without it,
    // nil — accept either so the test is build-agnostic.
    let src = r#"
(set 'r (date-parse "2002-02-27 18:21:30" "%Y-%m-%d %H:%M:%S"))
(println (or (= r 1014834090) (nil? r)))
(exit)
"#;
    let out = run("date-parse", src);
    assert!(out.contains("true"), "date-parse round-trip:\n{}", out);
}
