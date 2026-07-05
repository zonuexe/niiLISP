# FFI build and packaging: default-on Cargo feature, system libffi, Unix-first

The `import`/FFI subsystem (ADR-0015) is introduced without giving up the pure,
safe, zero-native-dependency character of the v0.1.0 crate.

> **Revised during implementation (2026-07).** The original decision was
> *bundled, statically linked* libffi. It does not build: `libffi-sys` 2.3.0's
> vendored libffi fails to assemble on current macOS (arm64, macOS 26.5 + clang)
> with CFI errors, so it cannot produce macOS binaries. The revised decision
> below uses the **system** libffi and scopes FFI to **Unix** for the first slice.

## Decisions

- **`import`/FFI lives behind a Cargo feature `ffi`, enabled by default**
  (`default = ["ffi"]`). The out-of-the-box `cargo install niilisp` runs
  `import`, honouring compatibility-first (ADR-0001). A pure build is available
  via `--no-default-features`. Reversing the default later would be a breaking
  change for downstream, hence it is fixed here.
- **Dependencies: `libloading`** (dlopen/dlsym) **and `libffi` with the `system`
  feature** (runtime-typed dynamic calls). Both are declared under
  `[target.'cfg(unix)'.dependencies]` and pulled in only by the `ffi` feature.
- **libffi is the system library.** On macOS it is OS-provided (in the SDK and
  `/usr/lib`), so a `-lffi` link resolves to the system copy and binaries stay
  portable across Macs. On Linux it is ubiquitous (a Python/GLib dependency); CI
  installs `libffi-dev` and the runtime `.so` is present on essentially all
  distros.
- **FFI is Unix-only in this slice.** The FFI code is gated on
  `#[cfg(all(feature = "ffi", unix))]`; on non-Unix targets the `ffi` feature
  still builds but compiles the FFI code out, so `cargo install niilisp` never
  breaks on Windows (it just lacks `import`). Windows FFI is deferred.

## Why not the alternatives

- **Bundled/static libffi (original choice):** would be self-contained, but the
  vendored source does not build on current macOS (see the revision note).
- **Opt-in (default off):** would hide the headline compatibility feature behind
  a flag, so the default `niilisp` would fail on newLISP scripts that use
  `import`. Rejected against priority #1.
- **Per-target (system on Unix, bundle on Windows):** the full-Windows-FFI path,
  but the bundle half is untestable here and adds Cargo complexity; deferred with
  Windows FFI.

## Consequences

- The default build requires the system libffi (`libffi-dev`/`brew install
  libffi` on machines that lack it). This is documented in the README.
- CI must exercise **both** configurations: default (`ffi`, with `libffi-dev`
  installed on Linux) and `--no-default-features` (the pure build must still
  compile and pass its tests).
- Release binaries: Unix targets build with `ffi` and dynamically link the
  system libffi (present on macOS/Linux); the **Windows binary builds with
  `--no-default-features`** (pure, no FFI) until Windows FFI lands.
- All `unsafe` FFI code is gated behind `#[cfg(all(feature = "ffi", unix))]` and
  confined to the FFI module (ADR-0015); the pure build remains 100% safe Rust.
- The published crate gains its first dependencies (Unix-only).

## Release matrix (revised 2026-07, before the first FFI release)

To ship `ffi`-enabled binaries on every Unix target without cross-compiling
(which fails to link the target's libffi), the release workflow builds each Unix
target on a **native runner** and installs the system libffi on Linux; Windows
builds pure. `cargo install niilisp` builds from source, so it already gets FFI
on Unix regardless of the prebuilt binaries — the goal here is only that the
convenience binaries carry the headline `import` feature where they can.

| target | runner (native) | build | libffi |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | default (`ffi`) | `apt-get install libffi-dev` |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | default (`ffi`) | `apt-get install libffi-dev` |
| `aarch64-apple-darwin` | `macos-latest` | default (`ffi`) | SDK-provided |
| `x86_64-pc-windows-msvc` | `windows-latest` | `--no-default-features` | — |

- **No cross-compilation.** `aarch64-linux` uses GitHub's native arm64 runner, so
  `-lffi` always resolves to the runner's own libffi.
- **No `x86_64-apple-darwin`.** v0.2.0 shipped four binaries; the Intel-macOS
  target was dropped afterwards — there is no real demand, and GitHub's `macos-13`
  (Intel) runners queue unreliably and are being retired. Intel-Mac users install
  from source (`cargo install`, which still gets FFI). Revisit only if demand
  appears.
- **Rejected: all binaries `--no-default-features`.** Simplest CI, but it drops
  `import` from every downloaded binary — a surprise for scripts that use it, and
  a poor fit for compatibility-first (ADR-0001). Native runners make FFI binaries
  cheap enough that this is not needed.
