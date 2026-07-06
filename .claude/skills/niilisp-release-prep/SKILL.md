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
which **first compiles every shipped target** (`build-check`), then — only if all
compiled — runs `cargo publish`, creates the GitHub Release from `CHANGELOG.md`,
and builds + uploads binaries. **Publishing is irreversible** (crates.io versions
can only be yanked, never replaced), so get a human Go before pushing the tag.

Prepare the release **on a `release/x.y.z` branch and open a PR**, so CI (which
now cross-checks the Windows target — see below) gates the exact release delta
before it touches `master`. **Squash-merge** the green PR, then tag the merged
commit.

At a glance: branch `release/x.y.z` -> bump version + seal changelog + reconcile
README -> verify locally -> push branch + open PR -> **CI green** -> **human Go**
-> squash-merge -> tag the merged commit + push -> Actions publishes.

### Why (a broken 0.3.0 shipped)

0.3.0 published to crates.io with a Windows-only compile error: a
`#[cfg(not(unix))]` branch that the Linux/macOS CI never compiles, and
`release.yml` published *before* building binaries, so the Windows failure came
after the irreversible publish. Two guards now prevent recurrence:

- `ci.yml` **cross-checks Windows** (`cargo check --target
  x86_64-pc-windows-msvc --no-default-features`) on every push/PR, so
  `cfg(windows)` / `cfg(not(unix))` branches are type-checked, not first seen at
  release time.
- `release.yml`'s **`build-check` gate** compiles all four targets on native
  runners and `publish-crate` `needs` it, so a target that can't build aborts the
  release *before* the publish.

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
cargo clippy --no-default-features --all-targets -- -D warnings
cargo check --target x86_64-pc-windows-msvc --no-default-features   # Windows compiles
cargo test                              # needs the references/newlisp submodule
cargo about generate about.hbs -o THIRD-PARTY-LICENSES.md           # regenerate deps' notices
cargo deny check licenses               # audit the dependency licenses
cargo doc --no-deps
cargo publish --dry-run                 # packages as crates.io will
git diff --check
```

Regenerate `THIRD-PARTY-LICENSES.md` **after** the version bump — `cargo about`
embeds niilisp's own entry with its version, so a stale file (or one generated
pre-bump) fails CI's drift guard.

`cargo publish --dry-run` confirms a small file count: `Cargo.toml`'s `exclude`
drops `.github`, `.claude`, agent docs, `/tests`, and the `references/newlisp`
submodule. If the package is large, fix `exclude` before publishing.

## Open the release PR, get CI + the Go, then tag to publish

Prepare on a branch and let CI gate it:

```sh
git switch -c release/x.y.z
# ... the bump / changelog / README / license-regen edits ...
git commit -am "Bump up version to x.y.z"
git push -u origin release/x.y.z
gh pr create --fill                       # CI runs on the PR (incl. Windows cross-check)
gh pr checks --watch                      # wait for green
```

If CI fails, fix on the branch and push again — nothing has touched `master` or
crates.io. When the PR is green, **stop and get a human Go** (the tag is the
irreversible publish). Only after the Go, squash-merge and tag the merged commit:

```sh
gh pr merge --squash                      # one clean commit on master
git switch master && git pull --ff-only
grep '^version' Cargo.toml                # sanity: equals x.y.z (release.yml re-checks)
git tag vx.y.z
git push origin vx.y.z                    # runs release.yml: build-check -> publish -> binaries + Release
gh run watch                              # watch the publish
```

`release.yml` compiles every target first, so a platform that can't build aborts
before the crates.io publish (see "Why" above).

## Verify the outcome

```sh
cargo search niilisp | head -1                              # newest = x.y.z
gh release view vx.y.z --json assets --jq '.assets[].name'  # 4 binaries attached
```

## Manual fallback (if Actions is unavailable)

```sh
cargo login                    # paste a crates.io token, once
cargo publish
git tag vx.y.z && git push origin vx.y.z
gh release create vx.y.z --title vx.y.z \
  --notes "$(awk -v v=x.y.z '$0 ~ "^## \\["v"\\]"{p=1;next} p&&/^## \\[/{exit} p' CHANGELOG.md)"
```
