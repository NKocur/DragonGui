#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: bash build_rpi_compat_wheel.sh [options]

Build a DragonGUI Raspberry Pi wheel on the target Pi so the wheel's
manylinux/glibc tag matches that Pi.

Options:
  --deps              Install Raspberry Pi OS build dependencies with apt.
  --build-root DIR    Local directory for venv, Cargo target, caches, and output.
  --copy-to DIR       Copy the built wheel into DIR after building.
  --install           Install the built wheel into the build venv for import check.
  -h, --help          Show this help.

Examples:
  bash build_rpi_compat_wheel.sh --deps
  bash build_rpi_compat_wheel.sh --build-root ~/.cache/dragongui-rpi-build
  bash build_rpi_compat_wheel.sh --copy-to ~/Desktop/Projects/WASP/py39
  bash build_rpi_compat_wheel.sh --deps --copy-to ~/Desktop/Projects/WASP/py39 --install
EOF
}

log() {
  printf '\n==> %s\n' "$*"
}

die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

install_deps=0
install_wheel=0
copy_to=""
build_root=""

while (($#)); do
  case "$1" in
    --deps)
      install_deps=1
      ;;
    --install)
      install_wheel=1
      ;;
    --build-root)
      shift
      [[ $# -gt 0 ]] || die "--build-root requires a directory"
      build_root="$1"
      ;;
    --copy-to)
      shift
      [[ $# -gt 0 ]] || die "--copy-to requires a directory"
      copy_to="$1"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "Unknown option: $1"
      ;;
  esac
  shift
done

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$script_dir"
cd "$repo_root"

[[ -f native/Cargo.toml ]] || die "native/Cargo.toml was not found. Copy/run this from the DragonGUI repo root."
[[ -f pyproject.toml ]] || die "pyproject.toml was not found. Copy/run this from the DragonGUI repo root."

if [[ -z "$build_root" ]]; then
  repo_key="$(printf '%s' "$repo_root" | tr -c 'A-Za-z0-9._-' '_')"
  build_root="${DRAGONGUI_BUILD_ROOT:-$HOME/.cache/dragongui-rpi-wheel/$repo_key}"
fi
mkdir -p "$build_root"
build_root="$(cd "$build_root" && pwd)"

if (( install_deps )); then
  log "Installing Raspberry Pi OS build dependencies"
  sudo apt update
  sudo apt install -y \
    build-essential \
    cmake \
    curl \
    pkg-config \
    python3-dev \
    python3-pip \
    python3-venv \
    libxkbcommon-dev \
    libwayland-dev \
    libxcb1-dev \
    libvulkan1 \
    mesa-vulkan-drivers
fi

log "Checking target platform"
python3 - <<'PY'
import platform
import sys

print("python:", sys.version.split()[0])
print("machine:", platform.machine())
if sys.version_info < (3, 9):
    raise SystemExit("Python 3.9+ is required")
if platform.machine() != "aarch64":
    raise SystemExit("This script must run on 64-bit Raspberry Pi OS/aarch64")
PY
ldd --version 2>&1 | sed -n '1p' || true

export DRAGONGUI_PROFILE="${DRAGONGUI_PROFILE:-pi}"
export DRAGONGUI_WGPU_BACKEND="${DRAGONGUI_WGPU_BACKEND:-gl}"
export DRAGONGUI_WINDOW_BACKEND="${DRAGONGUI_WINDOW_BACKEND:-x11}"
export RUSTUP_HOME="${RUSTUP_HOME:-$build_root/rustup}"
export CARGO_HOME="${CARGO_HOME:-$build_root/cargo}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$build_root/cargo-target}"
export PIP_CACHE_DIR="${PIP_CACHE_DIR:-$build_root/pip-cache}"
export PATH="$CARGO_HOME/bin:$PATH"

log "Using local build root: $build_root"

if ! command -v cargo >/dev/null 2>&1; then
  command -v curl >/dev/null 2>&1 || die "curl is missing. Re-run with --deps or install curl."
  log "Installing Rust stable toolchain"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain stable
  export PATH="$CARGO_HOME/bin:$PATH"
fi

log "Rust toolchain"
cargo --version
rustc --version

build_venv="$build_root/.wheel-build-venv"
if [[ ! -d "$build_venv" ]]; then
  log "Creating build virtual environment"
  python3 -m venv "$build_venv"
fi

# shellcheck disable=SC1091
. "$build_venv/bin/activate"

log "Installing Python build tools"
python -m pip install --upgrade pip 'maturin>=1.8,<2'

out_dir="$build_root/dist-rpi-compat"
mkdir -p "$out_dir"

log "Building target-compatible wheel"
maturin build --release --manifest-path native/Cargo.toml \
  --features pyo3/extension-module,pi \
  --out "$out_dir"

wheel="$(find "$out_dir" -maxdepth 1 -type f -name 'dragongui-*.whl' -printf '%T@ %p\n' | sort -n | tail -n 1 | sed 's/^[^ ]* //')"
[[ -n "$wheel" ]] || die "Build completed but no dragongui wheel was found in $out_dir"

log "Built wheel"
ls -lh "$wheel"

if (( install_wheel )); then
  log "Installing wheel into build venv and checking import"
  python -m pip install --force-reinstall --no-deps "$wheel"
  python - <<'PY'
import dragongui
print("dragongui:", dragongui.__file__)
print("native_backend_available:", dragongui.native_available())
PY
fi

if [[ -n "$copy_to" ]]; then
  mkdir -p "$copy_to"
  log "Copying wheel to $copy_to"
  cp "$wheel" "$copy_to/"
  ls -lh "$copy_to/$(basename "$wheel")"
fi

cat <<EOF

Done.

Wheel:
  $wheel

Install it on this Pi with:
  python3 -m pip install --force-reinstall --break-system-packages "$wheel"
EOF
