# Releasing DragonGUI

Production releases are built in GitHub Actions from immutable `v*` tags and
published with PyPI Trusted Publishing. Do not upload workstation-built files.

## One-time repository configuration

1. Create GitHub environments named `testpypi` and `pypi`.
2. Require an approving reviewer for the `pypi` environment.
3. Register `.github/workflows/release.yml` as a Trusted Publisher:
   - Owner: `NKocur`
   - Repository: `DragonGui`
   - TestPyPI environment: `testpypi`
   - PyPI environment: `pypi`
4. Protect release tags and the `master` branch.

## Release candidate

1. Ensure `CHANGELOG.md` contains the release and all package-owned version
   declarations agree.
2. Run:

   ```bash
   python tools/check_release.py
   python -m pytest
   cargo test --locked --manifest-path native/Cargo.toml
   cargo fmt --manifest-path native/Cargo.toml --all -- --check
   cargo clippy --locked --manifest-path native/Cargo.toml --lib -- -D clippy::correctness -A clippy::approx_constant -A dead-code
   cargo audit --file native/Cargo.lock
   ```

3. Run the `Release` workflow manually. It builds and validates the complete
   artifact matrix, then publishes to TestPyPI through the `testpypi`
   environment.
4. Install and smoke-test the TestPyPI release on each supported platform.

## Production release

1. Confirm the release commit is clean, reviewed, and passing CI.
2. Create and push the matching annotated tag:

   ```bash
   git tag -a v1.0.0 -m "DragonGUI 1.0.0"
   git push origin v1.0.0
   ```

3. The tag-triggered `Release` workflow rebuilds and validates every artifact.
4. Approve the protected `pypi` deployment after reviewing artifact checks.
5. Verify the PyPI page, install commands, project links, license, notices,
   GitHub release assets, and hashes.

PyPI versions are immutable. If a production upload is wrong, fix the issue,
increment the version, and publish a new release.
