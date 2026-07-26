# Third-Party Notices

DragonGUI's own source is licensed under the MIT License. Release artifacts
must also include license notices for bundled third-party Rust and Python
dependencies.

Before publishing a wheel or source distribution, regenerate and review the
third-party dependency notice output with an automated license tool such as
`cargo-about`.

Recommended release check:

```powershell
cargo about generate --manifest-path native/Cargo.toml --workspace > THIRD_PARTY_NOTICES.md
```

If the project adds dependencies with file-level copyleft licenses, such as
MPL-2.0 dependencies used unmodified through Cargo, keep their license notices
in the release artifact. DragonGUI source files remain MIT unless an
MPL-licensed source file is vendored or modified directly.
