"""Validate synchronized release metadata and built distributions."""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import tarfile
import tomllib
import zipfile
from email.parser import BytesParser
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_NOTICES = {"THIRD_PARTY_NOTICES.md", "THIRD_PARTY_RUST_NOTICES.md"}


def _toml(path: Path) -> dict[str, object]:
    with path.open("rb") as stream:
        return tomllib.load(stream)


def _quoted_version(path: Path, pattern: str) -> str:
    match = re.search(pattern, path.read_text(encoding="utf-8"), re.MULTILINE)
    if match is None:
        raise RuntimeError(f"could not find version in {path.relative_to(ROOT)}")
    return match.group(1)


def _check_source() -> str:
    project = _toml(ROOT / "pyproject.toml")["project"]
    assert isinstance(project, dict)
    version = str(project["version"])

    versions = {
        "pyproject.toml": version,
        "native/Cargo.toml": str(_toml(ROOT / "native" / "Cargo.toml")["package"]["version"]),
        "python/dragongui/__init__.py": _quoted_version(
            ROOT / "python" / "dragongui" / "__init__.py",
            r'^__version__\s*=\s*"([^"]+)"',
        ),
        "python/dragongui/manual.py": _quoted_version(
            ROOT / "python" / "dragongui" / "manual.py",
            r'^_LIBRARY_VERSION_FALLBACK\s*=\s*"([^"]+)"',
        ),
        "docs/sphinx/conf.py": _quoted_version(
            ROOT / "docs" / "sphinx" / "conf.py",
            r'^\s*release\s*=\s*"([^"]+)"',
        ),
    }
    mismatches = {path: value for path, value in versions.items() if value != version}
    if mismatches:
        raise RuntimeError(f"version mismatch: expected {version}, found {mismatches}")

    changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    if not re.search(rf"^## \[{re.escape(version)}\] - \d{{4}}-\d{{2}}-\d{{2}}$", changelog, re.MULTILINE):
        raise RuntimeError(f"CHANGELOG.md has no dated {version} release heading")

    if (ROOT / "native" / ".cargo" / "config.toml").exists():
        raise RuntimeError("machine-specific native/.cargo/config.toml must not be released")

    tracked_cache = subprocess.run(
        ["git", "ls-files", ".test-cache"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if tracked_cache:
        raise RuntimeError("generated .test-cache files are tracked")

    for notice in REQUIRED_NOTICES:
        path = ROOT / notice
        if not path.is_file() or path.stat().st_size < 500:
            raise RuntimeError(f"missing or incomplete {notice}")

    tag = os.environ.get("GITHUB_REF_NAME", "")
    if tag.startswith("v") and tag[1:] != version:
        raise RuntimeError(f"tag {tag} does not match package version {version}")

    print(f"source release metadata is synchronized at {version}")
    return version


def _wheel_metadata(path: Path) -> tuple[str, set[str]]:
    with zipfile.ZipFile(path) as archive:
        names = set(archive.namelist())
        metadata_name = next(name for name in names if name.endswith(".dist-info/METADATA"))
        metadata = BytesParser().parsebytes(archive.read(metadata_name))
    return str(metadata["Version"]), names


def _check_artifacts(directory: Path, version: str) -> None:
    wheels = sorted(directory.glob("*.whl"))
    sdists = sorted(directory.glob("*.tar.gz"))
    if not wheels or len(sdists) != 1:
        raise RuntimeError(f"expected wheels and one sdist in {directory}")

    for wheel in wheels:
        artifact_version, names = _wheel_metadata(wheel)
        if artifact_version != version:
            raise RuntimeError(f"{wheel.name} contains version {artifact_version}")
        missing = {notice for notice in REQUIRED_NOTICES if notice not in names}
        if missing:
            raise RuntimeError(f"{wheel.name} is missing {sorted(missing)}")

    with tarfile.open(sdists[0], "r:gz") as archive:
        names = {Path(name).name for name in archive.getnames()}
    required_sdist = {
        "CHANGELOG.md",
        "LICENSE",
        "RELEASING.md",
        "pyproject.toml",
        *REQUIRED_NOTICES,
    }
    if missing := required_sdist - names:
        raise RuntimeError(f"{sdists[0].name} is missing {sorted(missing)}")

    print(f"validated {len(wheels)} wheel(s) and {sdists[0].name}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifacts", type=Path)
    args = parser.parse_args()
    version = _check_source()
    if args.artifacts is not None:
        _check_artifacts(args.artifacts.resolve(), version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
