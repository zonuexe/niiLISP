//! Protocol-layer tests for the `gs:` GUI module (ADR-0034). The GUI itself is
//! display-dependent and not CI-automatable; these check that the vendored
//! `lib/gui.lsp` formats the wire commands the `niilisp-gui` helper expects —
//! no helper, no display.

use std::process::Command;

/// Run a script (with `lib/gui.lsp` loadable from the crate root) and return
/// stdout.
fn run(src: &str) -> String {
    let bin = env!("CARGO_BIN_EXE_niilisp");
    let dir = std::env::temp_dir().join(format!("niilisp-gui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("g.lsp");
    std::fs::write(&script, src).unwrap();
    let out = Command::new(bin).arg(&script).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn gs_command_builders_format_the_wire_protocol() {
    // `SGVsbG8=` / `T0s=` / `TmFtZTo=` are base64 of "Hello" / "OK" / "Name:".
    let src = r#"
(load "lib/gui.lsp")
(println "F|" (gs:frame-cmd 'W 10 20 300 200 "Hello"))
(println "B|" (gs:button-cmd 'B 'on-click "OK"))
(println "L|" (gs:label-cmd 'L "Name:"))
(println "T|" (gs:text-field-cmd 'T 'nil))
(println "S|" (gs:set-text-cmd 'L "Hi"))
(println "G|" (gs:set-background-cmd 'W 240 240 240))
(println "V|" (gs:set-visible-cmd 'W true))
(exit)
"#;
    let out = run(src);
    for (tag, expect) in [
        ("F|", "frame W 10 20 300 200 SGVsbG8="),
        ("B|", "button B on-click T0s="),
        ("L|", "label L TmFtZTo="),
        ("T|", "text-field T nil"),
        ("G|", "set-background W 240 240 240"),
        ("V|", "set-visible W 1"),
    ] {
        assert!(
            out.contains(&format!("{}{}", tag, expect)),
            "expected `{}{}` in:\n{}",
            tag,
            expect,
            out
        );
    }
}
