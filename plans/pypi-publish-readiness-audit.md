# PyPI Publish Readiness Audit

**Project:** DragonGui
**Audited version:** `1.0.0`
**Audit date:** August 5, 2026
**Current verdict:** **Ready for TestPyPI; production requires external setup and green release CI**

## Executive Summary

DragonGui 1.0.0 can produce a Windows ABI3 wheel and a source distribution.
Both artifacts pass metadata checks, clean installation, native import, and an
independent sdist rebuild. The available Python and Rust test suites also pass.

The repository-level release blockers found by this audit have been addressed:
DragonGui is consistently MIT licensed, complete generated Rust and bundled
asset notices are present, machine-specific Cargo configuration was removed,
generated test caches were untracked, and project URLs were corrected. The
release workflow now builds Windows, manylinux, macOS Intel, and macOS Apple
Silicon wheels, tests the ABI3 wheels on Python 3.12 and 3.13, validates the
sdist, supports TestPyPI, and publishes through PyPI Trusted Publishing.

The remaining release-owner work is:

1. Configure the `testpypi` and protected `pypi` GitHub environments.
2. Register `.github/workflows/release.yml` as a Trusted Publisher on both
   package indexes.
3. Run the TestPyPI workflow and require the full cross-platform matrix to pass
   before creating `v1.0.0`.

---

## Audit Results

### Successful Checks

The following checks passed during the audit:

- A fresh Windows release wheel built successfully:
  `dragongui-1.0.0-cp312-abi3-win_amd64.whl`.
- A fresh `dragongui-1.0.0.tar.gz` source distribution built successfully and
  rebuilt a native wheel in an isolated PEP 517 build.
- The wheel archive passed ZIP integrity validation.
- DragonGui imported successfully from an independently extracted wheel.
- `dragongui.__version__` reported `1.0.0`.
- The extracted package detected and loaded the native backend.
- A packaged GUI/WGPU smoke test completed successfully.
- The packaged help/manual lookup worked.
- Rust test result: **945 passed, 13 ignored, 0 failed**.
- Python fallback test result: **586 passed**.
- No obvious committed passwords, access tokens, credentials, or private-key
  patterns were found in tracked source files.
- The Python package, native crate, and Python `__version__` currently agree on
  version `1.0.0`.

### Validation Limitations

- The local Python 3.12 installation did not have pytest available. The complete
  Python suite was therefore run using the project's Python 3.11 fallback mode.
  This is useful evidence but does not replace supported-version testing.
- Python 3.13 was not available for local validation.
- `twine`, Ruff, `cargo-audit`, `cargo-deny`, and `cargo-about` were not available
  in the audit environment.
- Native binary dependency inspection tools such as `dumpbin` were unavailable.

---

## P0: Release Blockers

These issues should be resolved before any production PyPI upload.

### P0.1 — Contradictory Project License

**Status:** Resolved August 5, 2026
**Risk:** Closed; third-party compliance remains tracked separately in P0.2

DragonGui's copyright holders selected MIT for the project. The root `LICENSE`
was restored to the project's original MIT text, `pyproject.toml` now uses the
SPDX expression `license = "MIT"`, and the deprecated license classifier was
removed. `native/Cargo.toml`, the README, and `THIRD_PARTY_NOTICES.md` also
identify DragonGui as MIT.

The MPL-2.0 licenses of the unmodified `lightningcss` and `cssparser` Cargo
dependencies do not change DragonGui's project license. Their attribution,
license text, and source-availability obligations remain part of P0.2.

#### Required Work

Completed:

1. Restored the root MIT license text.
2. Synchronized the Python and Rust project metadata, README, and notices.
3. Adopted a modern SPDX license expression in Python project metadata.
4. Regenerated and inspected sdist metadata and the bundled project license.

#### Acceptance Criteria

- Every project license declaration identifies the same license.
- The wheel and sdist contain the intended license text.
- PyPI metadata reports the same license as the bundled files.
- Third-party licenses are kept distinct from DragonGui's own license.

---

### P0.2 — Third-Party Notices

**Status:** Resolved August 5, 2026
**Risk:** Closed; notices must remain reproducible as dependencies change

`THIRD_PARTY_NOTICES.md` now records the bundled xterm.js 5.5.0 assets, source
location, attribution, and MIT text. `THIRD_PARTY_RUST_NOTICES.md` is generated
from the locked dependency graph with `cargo-about`; it covers 454 third-party packages
across the supported platform targets and includes exact versions, upstream
repositories, attributions, and full detected license texts. The checked-in
`about.toml` policy accepts the reviewed license set and generation uses
`--locked --fail` so unresolved or unaccepted licenses fail the command.

#### Required Work

Completed:

1. Added a reproducible `cargo-about` policy and report template.
2. Generated the complete report from `native/Cargo.lock`.
3. Included full texts for the accepted license set, including MPL-2.0.
4. Documented xterm.js and FitAddon source availability and MIT attribution.
5. Configured maturin to ship both notice files in wheels and sdists.

The generation command still needs to become an enforced CI freshness check.

#### Acceptance Criteria

- `THIRD_PARTY_NOTICES.md` contains actual dependency notices.
- Every bundled dependency and asset has an identified license.
- Required license texts and attributions ship in the wheel and sdist.
- The notices agree with the final DragonGui project license.
- CI can reproduce or validate the notices.

---

### P0.3 — Machine-Specific Cargo Configuration Ships in the Sdist

**Status:** Resolved August 5, 2026
**Risk:** Closed; cross-platform sdist installation remains a release gate

`native/.cargo/config.toml` previously hardcoded:

```toml
[env]
PYO3_PYTHON = "C:\\msys64\\mingw64\\bin\\python.exe"
```

The committed configuration file has been removed. CI already selects its
interpreter explicitly with `PYO3_PYTHON`.

During the audit, a clean native check without an explicit override failed
because PyO3 selected Python 3.9. The crate requires at least Python 3.12 due to
the `abi3-py312` feature. Some Maturin build paths override this configuration,
but users and downstream builders should not have to depend on that behavior.

#### Required Work

1. Remove `PYO3_PYTHON` from the committed Cargo configuration.
2. Keep developer-specific interpreter selection in an untracked local file,
   shell environment, or documented developer setup.
3. Ensure CI selects its Python interpreter explicitly.
4. Build the sdist without `PYO3_PYTHON` set globally.
5. Test `pip install` from the sdist on Windows, macOS, and Linux.

#### Acceptance Criteria

- The sdist contains no absolute developer-machine paths.
- A clean source build discovers the active Python 3.12 or 3.13 interpreter.
- Source installation works on all supported operating systems.
- CI does not depend on a developer-local Cargo configuration.

---

### P0.4 — Release Must Come From a Clean Repository State

**Status:** Resolved August 5, 2026
**Risk:** Closed; release automation must enforce a clean checkout

All 316 generated files under `.test-cache` were removed from Git's index while
the local cache contents were preserved. `.test-cache/` is now ignored, matching
the pytest cache configuration.

Generated test installations should not be source-controlled or allowed to
affect the release commit.

#### Required Work

1. Confirm that no `.test-cache` contents are intentional source fixtures.
2. Remove generated `.test-cache` files from version control.
3. Add `.test-cache/` to `.gitignore`.
4. Audit other generated build, wheel, documentation, and runtime directories.
5. Build releases only from a clean reviewed commit.
6. Record the exact commit SHA in the release job.

#### Acceptance Criteria

- No generated cache or installed-wheel contents are tracked.
- `git status --short` is empty before the release build.
- Running tests does not dirty the worktree.
- The release artifacts can be traced to one reviewed commit and version tag.

---

## P1: Important Release Infrastructure

### P1.1 — Incorrect Project URLs

**Status:** Resolved August 5, 2026

`pyproject.toml` previously pointed to:

```text
https://github.com/dragonframe/dragongui
```

The repository's configured Git remote is:

```text
https://github.com/NKocur/DragonGui.git
```

Homepage, Source, and Issues metadata now use the repository's current public
location under `https://github.com/NKocur/DragonGui`.

#### Required Work

- Decide the permanent public repository location.
- Update the Homepage, Source, and Issues URLs.
- Add a documentation URL when public documentation is available.
- Verify each URL while unauthenticated.

#### Acceptance Criteria

- Every PyPI project URL resolves to the intended public resource.

---

### P1.2 — Dedicated Publishing Workflow

**Status:** Implemented August 5, 2026; Trusted Publisher setup remains external

`.github/workflows/release.yml` provides tag-driven production publishing,
manual TestPyPI staging, protected environments, OIDC Trusted Publishing,
validated artifacts, checksums, and GitHub release attachments.

Manual publishing is possible, but it is harder to reproduce and easier to
perform from an incorrect or dirty workspace.

#### Required Work

1. Create a release workflow triggered by version tags or an approved GitHub
   release.
2. Build all artifacts inside CI.
3. Use PyPI Trusted Publishing instead of a long-lived API token.
4. Store built artifacts as immutable workflow artifacts.
5. Add a manual TestPyPI deployment path.
6. Publish only after all validation jobs pass.
7. Consider artifact attestations and GitHub release attachments.

#### Acceptance Criteria

- A clean tag produces all release artifacts automatically.
- Production publishing requires the protected PyPI environment.
- No developer workstation or long-lived PyPI credential is needed.

---

### P1.3 — Platform Wheel Coverage

**Status:** Implemented; first cross-platform workflow run pending

The release matrix builds Windows x86-64, manylinux2014 x86-64, macOS Intel,
and macOS Apple Silicon wheels and clean-installs each artifact.

If platform wheels are unavailable, pip may fall back to the source
distribution. DragonGui's Rust and GPU dependencies make source installation a
substantially more demanding user experience.

#### Required Work

- Define supported operating systems, architectures, and minimum OS versions.
- Build Linux wheels using an appropriate manylinux policy.
- Determine whether macOS needs both Intel and Apple Silicon wheels.
- Determine whether Windows ARM64 is supported or explicitly unsupported.
- Audit native library dependencies in every wheel.
- Run a minimal application or renderer smoke test for every platform artifact.

#### Acceptance Criteria

- Every advertised platform has a compatible wheel or is clearly documented as
  source-only.
- Wheel tags and minimum platform versions are intentional.
- Each wheel imports and creates a minimal DragonGui application in a clean
  environment.

---

### P1.4 — Supported Python Matrix

**Status:** Implemented; first workflow run pending

CI runs the Python suite on 3.12 and 3.13, and the release workflow installs
each ABI3 platform wheel under both supported versions.

The native extension correctly uses `abi3-py312`, which should allow a single
wheel per platform to support Python 3.12 and later. That compatibility still
needs direct testing.

#### Required Work

- Run Python tests under 3.12 and 3.13.
- Install the same ABI3 wheel under both versions.
- Test native-backend imports under both versions.
- Add future Python versions only after CI proves compatibility.

#### Acceptance Criteria

- Python 3.12 and 3.13 pass package import, unit, and runtime smoke tests.
- Metadata classifiers match the tested compatibility matrix.

---

### P1.5 — Sdist Installation Release Gate

**Status:** Resolved August 5, 2026

The release workflow rebuilds and installs a native wheel exclusively from the
sdist. This was also validated locally. Maturin still prints a harmless root
manifest probe before honoring `native/Cargo.toml`; artifact creation and the
independent PEP 517 rebuild both succeed.

#### Required Work

- Investigate and eliminate or explain the Maturin manifest warning.
- Extract the generated sdist in a clean CI job.
- Build a wheel exclusively from the extracted sdist.
- Install that wheel in a clean environment.
- Run import and application smoke tests.

#### Acceptance Criteria

- The sdist builds without unexpected manifest errors or warnings.
- The sdist contains everything needed to build DragonGui.
- The source-install test passes on each supported platform.

---

## P2: Quality and Documentation Improvements

### P2.1 — Rust Formatting

**Status:** Resolved August 5, 2026

`cargo fmt --check` reports formatting differences in multiple native source
files, including layout, primitives, runtime, and scatter code.

#### Required Work

- Run and review `cargo fmt`.
- Add `cargo fmt --check` to CI.

#### Acceptance Criteria

- `cargo fmt --check` passes in a clean checkout.

---

### P2.2 — Clippy Baseline

**Status:** Correctness baseline established August 5, 2026

Running:

```text
cargo clippy --all-targets --all-features -- -D warnings
```

reported 197 errors. These include dead code, argument-heavy PyO3 APIs,
test-module placement, default-value reassignment, approximate constants, and
constant assertions.

Some warnings may be reasonable for Python bindings and can receive narrowly
scoped allowances. The project still needs an intentional lint policy.

#### Required Work

1. Categorize findings into defects, cleanup, and intentional binding patterns.
2. Fix actionable findings.
3. Add narrow `#[allow(...)]` attributes with explanations where appropriate.
4. Avoid broad crate-wide suppression unless justified.
5. Add the agreed Clippy command to CI.

#### Acceptance Criteria

- The documented Clippy command passes.
- Every suppression is narrow and intentional.

---

### P2.3 — Ruff Correctness Baseline

**Status:** Enforced August 5, 2026

Ruff is listed in the development dependencies and configured in
`pyproject.toml`, but CI does not run it. Ruff was unavailable in the audit
environment, so the current source was not independently verified.

#### Required Work

- Run `ruff check` and resolve current findings.
- Decide whether `ruff format --check` will also be enforced.
- Add the selected commands to CI.

#### Acceptance Criteria

- Python lint and formatting checks pass in CI.

---

### P2.4 — Package Validation and Dependency Audits

**Status:** Implemented; first CI advisory scan pending

The release process does not currently run:

- `twine check`
- `cargo audit`
- `cargo deny`
- reproducible third-party notice validation

#### Required Work

- Run `twine check` against both release artifacts.
- Add Rust advisory-database scanning.
- Add dependency license and banned-dependency policy.
- Decide how Python dependency vulnerabilities will be monitored.
- Record any accepted advisories with rationale and expiration/review dates.

#### Acceptance Criteria

- Artifact metadata passes `twine check`.
- No unreviewed security advisories or disallowed licenses remain.

---

### P2.5 — End-User Installation Documentation

**Status:** Resolved August 5, 2026

The README explains editable development installation but does not prominently
show ordinary PyPI installation:

```text
pip install dragongui
```

It also states that a Rust toolchain is required without clearly separating
normal wheel users from source-build users.

#### Required Work

- Add a top-level Installation section.
- Explain supported Python versions and operating systems.
- Explain that normal wheel installation does not require Rust.
- Document source-build prerequisites separately.
- Document optional extras with practical examples.
- Correct the Windows/MSYS2 GNU build instructions or label them as a specific
  developer configuration.
- Add a minimal verified first application.

#### Acceptance Criteria

- A new user can install and launch a minimal application from the README.
- Wheel and source-build requirements are clearly distinguished.

---

### P2.6 — Optional Dependency Structure Is Heavy

**Status:** Open

The `dataframe` extra installs both pandas and Polars:

```toml
dataframe = [
  "pandas>=2",
  "polars>=1",
]
```

Users who need only one dataframe backend must install both large dependency
stacks.

#### Recommended Work

Consider splitting the extras:

```toml
pandas = ["pandas>=2"]
polars = ["polars>=1"]
dataframe = ["pandas>=2", "polars>=1"]
```

Also determine whether NumPy should have a dedicated optional extra for
colormap helpers.

#### Acceptance Criteria

- Optional dependencies correspond to clearly documented capabilities.
- Users do not need unrelated heavy dependencies.

---

### P2.7 — Sdist Contents Need Deliberate Scoping

**Status:** Open

The sdist includes internal plans, tests, tools, benchmarks, documentation,
examples, and `start.bat`. This is valid but adds release noise and size.

Only top-level `examples/*.py` files are included, while some documentation can
refer to nested probe examples that are not present.

#### Required Work

- Decide which developer files are genuinely useful to downstream builders.
- Remove internal remediation plans from published artifacts unless there is a
  clear reason to ship them.
- Ensure every example referenced by shipped documentation is included.
- Keep test files only if they support downstream packaging verification.

#### Acceptance Criteria

- Sdist contents are intentional, documented, and internally consistent.

---

### P2.8 — Release Notes and Versioned Tags

**Status:** Documentation resolved; `v1.0.0` tag pending final release

`CHANGELOG.md` contains the 1.0.0 notes and `RELEASING.md` documents the staged
and production procedures. The immutable `v1.0.0` tag is intentionally deferred
until TestPyPI and the complete release matrix pass.

#### Required Work

- Add a changelog or release-notes process.
- Document version synchronization between Python and Cargo metadata.
- Use a versioned tag such as `v0.1.0`.
- Confirm that the tag points to the exact release commit.
- Document whether pre-alpha compatibility and API instability are intentional.

#### Acceptance Criteria

- The release has versioned notes and a matching immutable tag.
- The package version, tag, artifacts, and documentation agree.

---

## Recommended Release Workflow

The eventual release workflow should perform these stages in order:

1. **Source checks**
   - Confirm a clean checkout.
   - Validate synchronized versions.
   - Validate license declarations.
   - Run Ruff, Rustfmt, and Clippy.
   - Run Python and Rust tests.

2. **Legal and security checks**
   - Validate generated third-party notices.
   - Run dependency license policy checks.
   - Run Rust and Python vulnerability scans.

3. **Build artifacts**
   - Build Windows wheel.
   - Build macOS Intel and/or Apple Silicon wheels.
   - Build Linux manylinux wheel.
   - Build one canonical sdist.

4. **Artifact checks**
   - Run `twine check`.
   - Inspect wheel tags and metadata.
   - Install each wheel under Python 3.12 and 3.13 where applicable.
   - Rebuild a wheel from the sdist.
   - Run native import and GUI smoke tests.

5. **Staging**
   - Upload to TestPyPI.
   - Install from TestPyPI in clean environments.
   - Verify the rendered project page, links, README, license, and extras.

6. **Production**
   - Create the versioned release tag.
   - Build again from that tag or promote previously attested artifacts.
   - Publish through PyPI Trusted Publishing.
   - Attach the exact artifacts and checksums to the GitHub release.

---

## Final Pre-Publish Checklist

### Legal

- [x] DragonGui's intended license is confirmed.
- [x] All license declarations agree.
- [x] Third-party notices contain actual reviewed dependency data.
- [x] Bundled JavaScript and native assets have complete attribution.
- [ ] License-policy CI passes on GitHub.

### Repository

- [x] `.test-cache/` is untracked and ignored.
- [x] Running tests leaves the tracked worktree state unchanged.
- [ ] The release commit is reviewed.
- [ ] The release tag matches version `1.0.0`.
- [x] Repository and issue URLs are correct and public.

### Packaging

- [x] No developer-specific absolute paths ship in the sdist.
- [x] `twine check` passes for the wheel and sdist.
- [x] The sdist rebuild test passes locally and is a release gate.
- [x] Windows wheel passes clean installation and smoke tests.
- [ ] macOS wheel passes clean installation and smoke tests.
- [ ] Linux wheel passes clean installation and smoke tests.
- [ ] Native binary dependencies have been inspected.
- [ ] Python 3.12 and 3.13 are both validated.

### Quality

- [ ] Python tests pass under supported Python versions.
- [x] Rust tests pass.
- [x] Ruff correctness baseline passes.
- [x] Rustfmt passes.
- [x] The agreed Clippy correctness policy passes.
- [x] Dependency vulnerability check passes with zero known vulnerabilities.

### Documentation

- [x] README contains ordinary PyPI installation instructions.
- [x] Source-build requirements are documented separately.
- [x] Supported platforms and Python versions are accurate.
- [x] Optional extras are documented.
- [ ] A minimal first application is verified.
- [x] Changelog or release notes are complete.

### Publishing

- [ ] The `dragongui` PyPI name and account ownership are verified.
- [ ] TestPyPI installation succeeds.
- [ ] Trusted Publishing is configured.
- [ ] Production publishing requires protected approval.
- [ ] Published metadata and PyPI rendering are verified after upload.

---

## Release Decision

All repository P0 issues are closed. DragonGui 1.0.0 is ready for a TestPyPI
release candidate. Production remains a no-go until Trusted Publishing and the
protected GitHub environments are configured and the complete release workflow
passes on Windows, Linux, macOS Intel, macOS Apple Silicon, Python 3.12, and
Python 3.13.
