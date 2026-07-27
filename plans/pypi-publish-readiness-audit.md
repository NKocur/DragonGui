# PyPI Publish Readiness Audit

**Project:** DragonGui  
**Audited version:** `0.1.0`  
**Audit date:** July 27, 2026  
**Current verdict:** **Not ready to publish**

## Executive Summary

DragonGui can produce a Windows ABI3 wheel and a source distribution, and the
packaged native application successfully passed an import and GUI smoke test.
The available Python and Rust test suites also pass.

The project should not be uploaded to PyPI yet. The published artifacts currently
contain contradictory licensing information, placeholder third-party notices,
and a machine-specific Cargo configuration. The repository also lacks a clean,
repeatable, cross-platform release process.

The highest-priority work is:

1. Resolve the project's actual license.
2. Generate and review complete third-party dependency notices.
3. Remove developer-specific build configuration from published sources.
4. Clean generated test artifacts out of version control.
5. Establish reproducible Windows, macOS, Linux, Python 3.12, and Python 3.13
   release validation.

---

## Audit Results

### Successful Checks

The following checks passed during the audit:

- A fresh Windows release wheel built successfully:
  `dragongui-0.1.0-cp312-abi3-win_amd64.whl`.
- A fresh `dragongui-0.1.0.tar.gz` source distribution built successfully.
- The wheel archive passed ZIP integrity validation.
- DragonGui imported successfully from an independently extracted wheel.
- `dragongui.__version__` reported `0.1.0`.
- The extracted package detected and loaded the native backend.
- A packaged GUI/WGPU smoke test completed successfully.
- The packaged help/manual lookup worked.
- Rust test result: **756 passed, 12 ignored, 0 failed**.
- Python fallback test result: **513 passed**.
- No obvious committed passwords, access tokens, credentials, or private-key
  patterns were found in tracked source files.
- The Python package, native crate, and Python `__version__` currently agree on
  version `0.1.0`.

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

**Status:** Open  
**Risk:** Legal and distribution blocker

The repository does not consistently identify DragonGui's license:

- The root `LICENSE` file contains the complete GNU General Public License,
  version 2.
- `pyproject.toml` uses the MIT license classifier.
- `native/Cargo.toml` declares `license = "MIT"`.
- The README says the project is MIT licensed.
- `THIRD_PARTY_NOTICES.md` says DragonGui's source is MIT licensed.

The generated wheel reproduces this contradiction: its package metadata
advertises MIT while the bundled license file contains GPLv2.

#### Required Work

1. Determine which license the copyright holders intend to use.
2. Confirm that all contributors and incorporated source permit that choice.
3. Replace the root `LICENSE` file if MIT is intended, or update all MIT
   declarations if GPLv2 is intended.
4. Synchronize:
   - `LICENSE`
   - `pyproject.toml`
   - `native/Cargo.toml`
   - `README.md`
   - `THIRD_PARTY_NOTICES.md`
   - documentation pages that mention licensing
5. Prefer a modern SPDX license expression in project metadata after the
   intended license is confirmed.
6. Rebuild the wheel and sdist, then inspect their license metadata and bundled
   license files.

#### Acceptance Criteria

- Every project license declaration identifies the same license.
- The wheel and sdist contain the intended license text.
- PyPI metadata reports the same license as the bundled files.
- Third-party licenses are kept distinct from DragonGui's own license.

---

### P0.2 — Third-Party Notices Are a Placeholder

**Status:** Open  
**Risk:** Legal compliance blocker

`THIRD_PARTY_NOTICES.md` currently contains instructions for generating notices
instead of the actual dependency license report. This placeholder is included
in both release artifact formats.

The wheel contains a Maturin-generated CycloneDX SBOM, which is useful, but the
project's documented release policy explicitly requires reviewed third-party
license notices. An SBOM alone does not replace those notices.

#### Required Work

1. Install and configure an appropriate Rust license-reporting tool, such as
   `cargo-about`.
2. Generate the complete dependency license report from `native/Cargo.lock`.
3. Review unknown, copyleft, source-available, or manually specified licenses.
4. Include required license texts and attribution.
5. Account for bundled JavaScript terminal assets and any other non-Cargo
   third-party content.
6. Decide whether Python optional dependencies need notices in the distribution
   or documentation.
7. Add an automated CI check that fails when notices are stale or incomplete.

#### Acceptance Criteria

- `THIRD_PARTY_NOTICES.md` contains actual dependency notices.
- Every bundled dependency and asset has an identified license.
- Required license texts and attributions ship in the wheel and sdist.
- The notices agree with the final DragonGui project license.
- CI can reproduce or validate the notices.

---

### P0.3 — Machine-Specific Cargo Configuration Ships in the Sdist

**Status:** Open  
**Risk:** Source-install and cross-platform build blocker

`native/.cargo/config.toml` hardcodes:

```toml
[env]
PYO3_PYTHON = "C:\\msys64\\mingw64\\bin\\python.exe"
```

This path is specific to one Windows/MSYS2 development environment, does not
exist on a normal user's machine, and is included in the source distribution.

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

**Status:** Open  
**Risk:** Reproducibility and release-integrity blocker

The repository tracks approximately 215 files under `.test-cache`, including
installed package files, wheel metadata, SBOM data, and test runtime artifacts.
The worktree also contains numerous tracked cache-file deletions.

`pyproject.toml` configures pytest to use `.test-cache`, but `.gitignore` ignores
`.pytest_cache` rather than `.test-cache`.

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

**Status:** Open

`pyproject.toml` points to:

```text
https://github.com/dragonframe/dragongui
```

The repository's configured Git remote is:

```text
https://github.com/NKocur/DragonGui.git
```

Incorrect links would appear directly on the PyPI project page.

#### Required Work

- Decide the permanent public repository location.
- Update the Homepage, Source, and Issues URLs.
- Add a documentation URL when public documentation is available.
- Verify each URL while unauthenticated.

#### Acceptance Criteria

- Every PyPI project URL resolves to the intended public resource.

---

### P1.2 — No Dedicated Publishing Workflow

**Status:** Open

The repository has CI but no tag-driven release workflow, TestPyPI workflow,
PyPI Trusted Publishing configuration, signing, or artifact attestation.

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

### P1.3 — Incomplete Platform Wheel Coverage

**Status:** Open

The README advertises Windows, macOS, and Linux. CI builds and imports wheels on
Windows and macOS, but no Linux wheel is built or exercised.

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

### P1.4 — Supported Python Matrix Is Not Fully Tested

**Status:** Open

The package advertises Python 3.12 and 3.13. CI currently exercises Python tests
only on 3.12, and the local full-suite audit used the fallback backend under
Python 3.11 because pytest was unavailable in the local Python 3.12 environment.

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

### P1.5 — Sdist Installation Is Not a CI Release Gate

**Status:** Open

The sdist was generated and could rebuild a wheel during the audit, but CI does
not validate this workflow. Maturin also emitted a suspicious message stating
that a root `Cargo.toml` did not exist while producing the sdist, even though
the artifact was ultimately produced.

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

### P2.1 — Rust Formatting Check Fails

**Status:** Open

`cargo fmt --check` reports formatting differences in multiple native source
files, including layout, primitives, runtime, and scatter code.

#### Required Work

- Run and review `cargo fmt`.
- Add `cargo fmt --check` to CI.

#### Acceptance Criteria

- `cargo fmt --check` passes in a clean checkout.

---

### P2.2 — No Established Clippy Baseline

**Status:** Open

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

### P2.3 — Ruff Is Configured but Not Enforced

**Status:** Open

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

### P2.4 — Missing Package Validation and Dependency Audits

**Status:** Open

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

### P2.5 — README Needs an End-User Installation Section

**Status:** Open

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

### P2.8 — Release Notes and Versioned Tags Are Missing

**Status:** Open

There is no changelog or dedicated release checklist. The repository has a
`DragonGui` tag but no semantic version tag for `0.1.0`.

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

- [ ] DragonGui's intended license is confirmed.
- [ ] All license declarations agree.
- [ ] Third-party notices contain actual reviewed dependency data.
- [ ] Bundled JavaScript and native assets have complete attribution.
- [ ] License-policy CI passes.

### Repository

- [ ] `.test-cache/` is untracked and ignored.
- [ ] Running tests leaves the worktree clean.
- [ ] The release commit is reviewed.
- [ ] The release tag matches version `0.1.0`.
- [ ] Repository and issue URLs are correct and public.

### Packaging

- [ ] No developer-specific absolute paths ship in the sdist.
- [ ] `twine check` passes for the wheel and sdist.
- [ ] The sdist rebuild test passes.
- [ ] Windows wheel passes clean installation and smoke tests.
- [ ] macOS wheel passes clean installation and smoke tests.
- [ ] Linux wheel passes clean installation and smoke tests.
- [ ] Native binary dependencies have been inspected.
- [ ] Python 3.12 and 3.13 are both validated.

### Quality

- [ ] Python tests pass under supported Python versions.
- [ ] Rust tests pass.
- [ ] Ruff passes.
- [ ] Rustfmt passes.
- [ ] The agreed Clippy policy passes.
- [ ] Dependency vulnerability checks pass.

### Documentation

- [ ] README contains ordinary PyPI installation instructions.
- [ ] Source-build requirements are documented separately.
- [ ] Supported platforms and Python versions are accurate.
- [ ] Optional extras are documented.
- [ ] A minimal first application is verified.
- [ ] Changelog or release notes are complete.

### Publishing

- [ ] The `dragongui` PyPI name and account ownership are verified.
- [ ] TestPyPI installation succeeds.
- [ ] Trusted Publishing is configured.
- [ ] Production publishing requires protected approval.
- [ ] Published metadata and PyPI rendering are verified after upload.

---

## Release Decision

DragonGui should remain in **no-go** status until all P0 issues are closed.

After the P0 issues are fixed, the project can move to a TestPyPI release
candidate. Production PyPI publishing should occur only after the platform
wheel matrix, supported Python versions, artifact validation, and clean
installation tests are complete.
