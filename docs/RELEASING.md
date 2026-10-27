# Releasing CamY

The installed plugin follows `main`; merging QML changes delivers them on the
next install/update. The helper is a separately published binary selected by
[helper/release.pin](../helper/release.pin). Helper source may be ahead of that
pin. Plugin and helper versions move independently.

Prepare and pin commands write locally on a working branch, never on `main`
or detached HEAD. They do not commit, merge, or push. Publication is an explicit
GitHub action after draft review. Published assets are immutable; a mistake
requires a new version.

## Helper

1. On a working branch, run `bash scripts/release.sh helper prepare <version>`
   with a greater X.Y.Z version. It changes only the helper package version in
   `helper/Cargo.toml` (and refreshes `helper/Cargo.lock`). Review, verify, and
   commit that change.
2. After that commit is on `main`, from clean local `main` matching remote
   `main`, run `bash scripts/release.sh helper tag`. The command creates the
   annotated `helper-<version>` tag. Push it with `git push github helper-<version>`
   after review (`--push` does that in one step).
3. [Helper release CI](../.github/workflows/helper-release.yml) builds native
   optimized x86_64 and aarch64 helpers, then packages both binaries,
   `SHA256SUMS`, build metadata, and a candidate `release.pin`. Only a tag push
   creates a draft; manual workflow runs produce candidate artifacts.
4. Review both native results, the draft assets and notes, then publish in
   GitHub. Do not modify assets after publishing.
5. On a working branch, run `bash scripts/release.sh helper pin helper-<version>`.
   It requires a public published release, downloads both complete executable
   ELF architectures, checksums them, and writes the tracked pin only after
   every check passes. Failure leaves the tracked pin unchanged.
6. Commit the verified pin and compatible QML together. The UI must never
   require an unpublished helper.

## Plugin

1. On a working branch, run `bash scripts/release.sh plugin prepare <version>`.
   Verify the change and commit the manifest version. Complete any new helper
   publication and compatible pin before shipping dependent UI.
2. After merge, run `bash scripts/release.sh plugin tag` from clean `main`.
   It creates `v<version>` locally. Push with `git push github v<version>`.
3. Publish the GitHub release for that tag after reviewing the notes.

## First helper release

The fetch path needs a public `helper-*` GitHub Release whose sha256 matches
the committed pin. Until that pin is written from published assets, a checkout
with `bin/camy` (from `scripts/build-helper.sh`) stays offline. Marketplace
users get the download on first open only after step 5 is committed.

## Release notes

Titles are the product and version only: `CamY 0.5.0` and `Helper 0.2.0`.
Name security fixes plainly.
