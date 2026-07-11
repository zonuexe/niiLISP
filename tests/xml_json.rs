//! XML and JSON parsing (ADR-0038): `xml-parse`/`xml-type-tags`/`xml-error` and
//! `json-parse`/`json-error`. Examples follow the newLISP manual.

use std::process::Command;

fn run(name: &str, src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-xj-{}-{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("p.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn xml_parse_matches_the_manual() {
    let src = r#"
(println (xml-parse "<person name='John Doe' tel='555-1212'>nice guy</person>"))
(exit)
"#;
    let out = run("xml", src);
    assert!(
        out.contains(
            r#"(("ELEMENT" "person" (("name" "John Doe") ("tel" "555-1212")) (("TEXT" "nice guy"))))"#
        ),
        "xml-parse:\n{}",
        out
    );
}

#[test]
fn xml_options_comments_cdata_and_entities() {
    let src = r#"
(println (xml-parse "<?xml version=\"1.0\"?><a><b>x</b></a>" 3))
(println (xml-parse "<r><!--hi--><![CDATA[<raw>]]></r>"))
(println (xml-parse "<a>1 &lt; 2 &amp; 3 &#65;</a>"))
(exit)
"#;
    let out = run("xml-opts", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(
        l.first().copied(),
        Some(r#"(("ELEMENT" "a" (("ELEMENT" "b" (("TEXT" "x"))))))"#),
        "options 3 (ws + empty attrs, skip prolog):\n{}",
        out
    );
    assert_eq!(
        l.get(1).copied(),
        Some(r#"(("ELEMENT" "r" () (("COMMENT" "hi") ("CDATA" "<raw>"))))"#),
        "comment + cdata:\n{}",
        out
    );
    assert_eq!(
        l.get(2).copied(),
        Some(r#"(("ELEMENT" "a" () (("TEXT" "1 < 2 & 3 A"))))"#),
        "entities:\n{}",
        out
    );
}

#[test]
fn xml_error_reports_on_malformed_input() {
    let src = r#"
(println (xml-parse "<atag>hello</atag><fin"))
(println (list? (xml-error)))
(xml-parse "<ok/>")
(println (xml-error))
(exit)
"#;
    let out = run("xml-error", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(l.first().copied(), Some("nil"), "malformed → nil:\n{}", out);
    assert_eq!(l.get(1).copied(), Some("true"), "error is a list:\n{}", out);
    assert_eq!(
        l.get(2).copied(),
        Some("nil"),
        "cleared on success:\n{}",
        out
    );
}

#[test]
fn json_parse_objects_arrays_and_literals() {
    let src = r#"
(println (json-parse "{\"name\": \"John\", \"age\": 30}"))
(println (json-parse "[1, 2.5, true, false, null, \"hi\"]"))
(set 'j (json-parse "{\"a\": {\"b\": [10, 20]}}"))
(println (lookup "a" j))
(exit)
"#;
    let out = run("json", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(
        l.first().copied(),
        Some(r#"(("name" "John") ("age" 30))"#),
        "object:\n{}",
        out
    );
    assert_eq!(
        l.get(1).copied(),
        Some(r#"(1 2.5 true false null "hi")"#),
        "array + literals:\n{}",
        out
    );
    assert_eq!(
        l.get(2).copied(),
        Some(r#"(("b" (10 20)))"#),
        "nested lookup:\n{}",
        out
    );
}

#[test]
fn json_error_reports_on_malformed_input() {
    let src = r#"
(println (json-parse "{\"a\" 1}"))
(println (list? (json-error)))
(json-parse "[1, 2, 3]")
(println (json-error))
(exit)
"#;
    let out = run("json-error", src);
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(l.first().copied(), Some("nil"), "malformed → nil:\n{}", out);
    assert_eq!(l.get(1).copied(), Some("true"), "error is a list:\n{}", out);
    assert_eq!(
        l.get(2).copied(),
        Some("nil"),
        "cleared on success:\n{}",
        out
    );
}
