# FFI build and packaging: default-on Cargo feature, bundled static libffi

The `import`/FFI subsystem (ADR-0015) is introduced without giving up the pure,
safe, zero-native-dependency character of the v0.1.0 crate.

## Decisions

- **`import`/FFI lives behind a Cargo feature `ffi`, enabled by default**
  (`default = ["ffi"]`). The out-of-the-box `cargo install niilisp` runs
  `import`, honouring compatibility-first (ADR-0001). A pure build is available
  via `--no-default-features` for targets where FFI cannot or should not exist
  (WASM sandbox, minimal or audited builds). Reversing the default later would be
  a breaking change for downstream, hence it is fixed here.
- **Dependencies: `libloading`** (runtime shared-library load + symbol
  resolution, dlopen/dlsym) **and `libffi`** (runtime-typed dynamic calls and
  closures for `callback`). Both are pulled in only under the `ffi` feature.
- **libffi is bundled and statically linked** — the `libffi` crate's default,
  which builds libffi from source via `libffi-sys`. The prebuilt release binaries
  (release.yml, 5 platforms) therefore stay **self-contained**: "download a binary
  and run" keeps working, with no runtime libffi dependency on the target.

## Why not the alternatives

- **Opt-in (default off):** would hide the headline compatibility feature behind
  a flag, so the default `niilisp` would fail on the many newLISP scripts that use
  `import`. Rejected against priority #1.
- **System libffi (pkg-config):** lighter build, but the prebuilt binaries would
  then depend on the target having libffi installed, breaking the standalone-binary
  distribution. Rejected.

## Consequences

- The default build requires a C compiler (to build bundled libffi) and takes
  longer; this is consistent with the default-on choice.
- CI must exercise **both** configurations: default (`--features ffi`, implied) and
  `--no-default-features` (pure build must still compile and pass its tests).
- All `unsafe` FFI code is gated behind `#[cfg(feature = "ffi")]` and confined to
  the FFI module (ADR-0015); the pure build remains 100% safe Rust.
- The published crate gains its first dependencies; `cargo publish --dry-run`
  package contents are otherwise unchanged.
