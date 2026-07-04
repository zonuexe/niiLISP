---
name: niilisp-release-prep
description: Prepare a niiLISP release - bump the crate version, seal the changelog, reconcile the README, run verification, and tag so GitHub Actions publishes to crates.io and attaches pre-built binaries. Use when the user asks to prepare the next version, cut a release, or make versioned files consistent before tagging.
metadata:
  internal: true
---

# niiLISP Release Prep

Follow this workflow to release a new `niilisp` version. It ships two ways from
one tag: the crate on [crates.io](https://crates.io/crates/niilisp) (`cargo
install niilisp`) and per-platform pre-built binaries on the GitHub Release.

The `vX.Y.Z` tag triggers [`release.yml`](../../../.github/workflows/release.yml),
which runs `cargo publish`, creates the GitHub Release from `CHANGELOG.md`, and
builds + uploads binaries. **Publishing is irreversible** (crates.io versions can
only be yanked, never replaced), so get a human Go before pushing the tag.

At a glance: bump version -> seal changelog -> reconcile README -> verify -> commit
-> **human Go** -> tag + push -> Actions publishes.

## One-time setup (already done)

- The crates.io API token is stored as the repository secret
  `CARGO_REGISTRY_TOKEN` (GitHub -> Settings -> Secrets and variables -> Actions).
- The binary job uses the built-in `GITHUB_TOKEN`; no extra secret needed.

## Update release metadata

Decide the next semantic version, then update all versioned files together:

- `Cargo.toml` - the `version` field.
- `Cargo.lock` - bump niilisp's own entry (`cargo build` refreshes it). This is a
  binary crate that **tracks `Cargo.lock`**, so it must stay in sync.
- `CHANGELOG.md` - seal `[Unreleased]` into the new version section.

### Seal the `[Unreleased]` entries

The changelog is for humans; make it read like release notes, not commit messages.
`release.yml` extracts the version's section verbatim as the GitHub Release body.

1. If `[Unreleased]` is thin, reconstruct it from `git log <last-tag>..HEAD --oneline`.
2. Rewrite each bullet to one self-contained, user-facing sentence; drop
   internal-only detail (private refactors, test additions).
3. Add a `## [x.y.z] - YYYY-MM-DD` section below `## [Unreleased]`, using Keep a
   Changelog headings (`Added`, `Changed`, `Fixed`, ...).
4. **Do not hard-wrap entries** - each bullet is one physical line (wrapping
   degrades the GitHub Release body).
5. Update the bottom links: point `[Unreleased]` at `compare/vx.y.z...HEAD` and add
   `[x.y.z]: https://github.com/zonuexe/niiLISP/releases/tag/vx.y.z`.

## Reconcile the README

`README.md` is the crates.io page. Before tagging, check it against the sealed
changelog and the real binary:

- **Usage/CLI** - the `## Usage` block matches the real flags (grep `src/main.rs`
  `USAGE`), and every option the binary exposes appears.
- **Status** - reflects what actually ships this release (no stale "not yet" for
  things now done); update which `qa-*` suites pass.
- **Examples** - the `examples/` scripts named still exist and run.

## Verify the release

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                              # needs the references/newlisp submodule
cargo doc --no-deps
cargo publish --dry-run                 # packages as crates.io will
git diff --check
```

`cargo publish --dry-run` confirms a small file count: `Cargo.toml`'s `exclude`
drops `.github`, `.claude`, agent docs, `/tests`, and the `references/newlisp`
submodule. If the package is large, fix `exclude` before publishing.

## Commit, get the Go, then tag to publish

One release-prep commit (`Bump up version to x.y.z`) with the `Cargo.toml`,
`Cargo.lock`, `CHANGELOG.md`, and any `README.md` edits. Push `master` and make sure
`ci.yml` is green.

Then **stop and get a human Go** - the tag push is the irreversible publish. Only
after the Go:

```sh
grep '^version' Cargo.toml     # sanity: equals x.y.z (release.yml re-checks)
git tag vx.y.z
git push origin vx.y.z         # runs release.yml -> crate + binaries + Release
gh run watch                   # watch the publish
```

## Verify the outcome

```sh
cargo search niilisp | head -1                              # newest = x.y.z
gh release view vx.y.z --json assets --jq '.assets[].name'  # 5 binaries attached
```

## Manual fallback (if Actions is unavailable)

```sh
cargo login                    # paste a crates.io token, once
cargo publish
git tag vx.y.z && git push origin vx.y.z
gh release create vx.y.z --title vx.y.z \
  --notes "$(awk -v v=x.y.z '$0 ~ "^## \\["v"\\]"{p=1;next} p&&/^## \\[/{exit} p' CHANGELOG.md)"
```
