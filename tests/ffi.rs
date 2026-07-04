//! Hermetic FFI integration test (ADR-0019): compile a tiny C shared library
//! with `cc` at test time, then `import` and call it. Only compiled/run when the
//! `ffi` feature is on and the target is Unix.
#![cfg(all(feature = "ffi", unix))]

use std::process::Command;

#[test]
fn import_and_call_c_functions() {
    let dir = std::env::temp_dir().join(format!("niilisp-ffi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t.c");
    std::fs::write(
        &src,
        "int add3(int x){return x+3;}\n\
         double half(double d){return d/2.0;}\n\
         const char* greet(void){return \"hi\";}\n\
         int apply_cb(int (*f)(int), int x){return f(x);}\n",
    )
    .unwrap();

    let (libname, kind) = if cfg!(target_os = "macos") {
        ("libt.dylib", "-dynamiclib")
    } else {
        ("libt.so", "-shared")
    };
    let lib = dir.join(libname);
    let status = Command::new("cc")
        .args(["-fPIC", kind])
        .arg(&src)
        .arg("-o")
        .arg(&lib)
        .status()
        .expect("failed to run cc");
    assert!(status.success(), "cc did not build the test library");

    let script = format!(
        "(import \"{lib}\" \"add3\" \"int\" \"int\")\n\
         (import \"{lib}\" \"half\" \"double\" \"double\")\n\
         (import \"{lib}\" \"greet\" \"char*\")\n\
         (import \"{lib}\" \"apply_cb\" \"int\" \"void*\" \"int\")\n\
         (define (dbl x) (* x 2))\n\
         (println \"add3=\" (add3 39))\n\
         (println \"half=\" (half 7.0))\n\
         (println \"greet=\" (greet))\n\
         (println \"cb=\" (apply_cb (callback 'dbl \"int\" \"int\") 21))\n",
        lib = lib.display()
    );

    let bin = env!("CARGO_BIN_EXE_niilisp");
    let output = Command::new(bin)
        .arg("-e")
        .arg(&script)
        .output()
        .expect("failed to run niilisp");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let _ = std::fs::remove_dir_all(&dir);

    assert!(stdout.contains("add3=42"), "int call failed:\n{}", stdout);
    assert!(stdout.contains("cb=42"), "callback failed:\n{}", stdout);
    assert!(
        stdout.contains("half=3.5"),
        "double call failed:\n{}",
        stdout
    );
    assert!(
        stdout.contains("greet=hi"),
        "char* return failed:\n{}",
        stdout
    );
}
