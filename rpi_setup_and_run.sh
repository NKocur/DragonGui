#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: bash rpi_setup_and_run.sh <command> [python-example]

Commands:
  deps         Install Raspberry Pi OS build/runtime apt packages.
  setup        Create the USB-backed Rust/Cargo dirs and Python venv.
  build        Build the native wheel, install it, and copy _dragongui*.so into python/dragongui.
  diag         Print Pi display/GPU diagnostics and run one-frame backend probes.
  smoke        Run a short Pi-profile smoke test. Defaults to examples/all_features_v3_demo.py.
  run          Run the demo normally. Defaults to examples/all_features_v3_demo.py.
  build-smoke  Build, install, copy the native extension, then run smoke. This is the default.
  build-run    Build, install, copy the native extension, then run normally.
  full         Install apt deps, build, install, copy the native extension, then run smoke.

Useful overrides:
  DRAGONGUI_USB_ROOT=/media/xymbu/DragonUSB
  DRAGONGUI_PROFILE=pi
  DRAGONGUI_WGPU_BACKEND=gl     # Script default; use vulkan to test Vulkan explicitly.
  DRAGONGUI_WINDOW_BACKEND=x11  # Script default for GL on Pi; use wayland to test Wayland explicitly.
  DRAGONGUI_SMOKE_FRAMES=3

Examples:
  bash rpi_setup_and_run.sh full
  bash rpi_setup_and_run.sh diag
  bash rpi_setup_and_run.sh build-smoke
  bash rpi_setup_and_run.sh run examples/css_feature_probes/line_plot_probe.py
EOF
}

log() {
  printf '\n==> %s\n' "$*"
}

die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="${DRAGONGUI_REPO_ROOT:-$script_dir}"

cd "$repo_root"
[[ -f native/Cargo.toml ]] || die "native/Cargo.toml was not found. Run this script from the DragonGUI repo root."

usb_root="${DRAGONGUI_USB_ROOT:-$(dirname "$repo_root")}"
export RUSTUP_HOME="${RUSTUP_HOME:-$usb_root/rustup}"
export CARGO_HOME="${CARGO_HOME:-$usb_root/cargo}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$usb_root/cargo-target}"
export PIP_CACHE_DIR="${PIP_CACHE_DIR:-$usb_root/pip-cache}"
export PIP_DISABLE_PIP_VERSION_CHECK="${PIP_DISABLE_PIP_VERSION_CHECK:-1}"
export DRAGONGUI_PROFILE="${DRAGONGUI_PROFILE:-pi}"
export DRAGONGUI_WGPU_BACKEND="${DRAGONGUI_WGPU_BACKEND:-gl}"
export DRAGONGUI_WINDOW_BACKEND="${DRAGONGUI_WINDOW_BACKEND:-x11}"
export PYTHONPATH="$repo_root/python${PYTHONPATH:+:$PYTHONPATH}"
export PATH="$CARGO_HOME/bin:$PATH"

apt_packages=(
  build-essential
  cmake
  pkg-config
  libvulkan1
  mesa-vulkan-drivers
  mesa-utils
  vulkan-tools
  libxkbcommon-dev
  libwayland-dev
  libxcb1-dev
  xdg-desktop-portal
  xdg-desktop-portal-gtk
  python3-dev
  python3-pip
  python3-venv
  curl
)

install_deps() {
  log "Installing Raspberry Pi OS packages"
  sudo apt update
  sudo apt install -y "${apt_packages[@]}"
}

prepare_usb_dirs() {
  log "Using USB/build root: $usb_root"
  mkdir -p "$RUSTUP_HOME" "$CARGO_HOME" "$CARGO_TARGET_DIR" "$PIP_CACHE_DIR"
}

ensure_rust() {
  prepare_usb_dirs
  if ! command -v cargo >/dev/null 2>&1; then
    command -v curl >/dev/null 2>&1 || die "curl is missing. Run: bash rpi_setup_and_run.sh deps"
    log "Installing Rust into $RUSTUP_HOME and $CARGO_HOME"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain stable
  fi

  if command -v rustup >/dev/null 2>&1; then
    rustup default stable
  fi

  cargo --version
  rustc --version
}

ensure_venv() {
  prepare_usb_dirs
  if [[ ! -d .venv ]]; then
    log "Creating Python virtual environment"
    python3 -m venv .venv
  fi

  activate_venv

  log "Installing Python build/runtime packages"
  python -m pip install --upgrade pip maturin numpy plotly
}

activate_venv() {
  [[ -d .venv ]] || die "Python virtual environment is missing. Run: bash rpi_setup_and_run.sh setup"
  # shellcheck disable=SC1091
  . .venv/bin/activate
}

build_wheel() {
  log "Building DragonGUI native wheel"
  maturin build --release --manifest-path native/Cargo.toml \
    --features pyo3/extension-module,pi
}

latest_wheel() {
  {
    find "$CARGO_TARGET_DIR/wheels" "$repo_root/target/wheels" \
      -maxdepth 1 -type f -name 'dragongui-*.whl' -printf '%T@ %p\n' 2>/dev/null || true
  } | sort -n | tail -n 1 | sed 's/^[^ ]* //'
}

install_wheel() {
  local wheel
  wheel="$(latest_wheel)"
  [[ -n "$wheel" ]] || die "No dragongui wheel found under $CARGO_TARGET_DIR/wheels or target/wheels."

  log "Installing wheel: $wheel"
  python -m pip install --force-reinstall "$wheel"
}

copy_native_extension() {
  local extension
  extension="$(find "$repo_root/.venv" -type f -name '_dragongui*.so' 2>/dev/null | sort | tail -n 1)"
  [[ -n "$extension" ]] || die "Installed _dragongui*.so was not found in .venv. Re-run the build command."

  log "Copying native extension into source package"
  mkdir -p "$repo_root/python/dragongui"
  cp "$extension" "$repo_root/python/dragongui/"
}

copy_native_extension_if_available() {
  local extension
  extension="$(find "$repo_root/.venv" -type f -name '_dragongui*.so' 2>/dev/null | sort | tail -n 1)"
  if [[ -n "$extension" ]]; then
    log "Copying native extension into source package"
    mkdir -p "$repo_root/python/dragongui"
    cp "$extension" "$repo_root/python/dragongui/"
  else
    printf 'Installed _dragongui*.so was not found in .venv; skipping native extension copy.\n'
  fi
}

verify_backend() {
  log "Verifying native backend import"
  python - <<'PY'
import dragongui as dg

info = dg.backend_info()
platform = info.get("platform") or {}
print("dragongui:", dg.__file__)
print("native:", info.get("native"))
print("profile:", platform.get("profile"))
print("backend override:", platform.get("wgpu_backend_override"))
if not info.get("native"):
    raise SystemExit("DragonGUI native backend is still unavailable.")
PY
}

optional_command() {
  local label="$1"
  shift
  log "$label"
  if command -v "$1" >/dev/null 2>&1; then
    "$@" || true
  else
    printf 'missing command: %s\n' "$1"
  fi
}

optional_shell() {
  local label="$1"
  shift
  log "$label"
  bash -lc "$*" || true
}

apply_window_backend_env() {
  case "$(printf '%s' "$DRAGONGUI_WINDOW_BACKEND" | tr '[:upper:]' '[:lower:]')" in
    x|x11)
      if [[ -z "${DISPLAY:-}" ]]; then
        die "DRAGONGUI_WINDOW_BACKEND=x11 requires DISPLAY to be set. Try DRAGONGUI_WINDOW_BACKEND=wayland or start an X11/XWayland session."
      fi
      unset WAYLAND_DISPLAY
      unset WAYLAND_SOCKET
      export DRAGONGUI_WINDOW_BACKEND=x11
      log "Using X11/XWayland window backend for GL surface compatibility"
      ;;
    wayland|wl)
      export DRAGONGUI_WINDOW_BACKEND=wayland
      log "Using Wayland window backend"
      ;;
    auto|"")
      unset DRAGONGUI_WINDOW_BACKEND
      log "Using automatic window backend selection"
      ;;
    *)
      die "Invalid DRAGONGUI_WINDOW_BACKEND=$DRAGONGUI_WINDOW_BACKEND; expected auto, x11, or wayland"
      ;;
  esac
}

print_backend_info() {
  log "DragonGUI backend_info()"
  python - <<'PY' || true
import json
import dragongui as dg

print("dragongui:", dg.__file__)
print(json.dumps(dg.backend_info(), indent=2, sort_keys=True))
PY
}

run_gpu_probe() {
  local backend="$1"
  local window_backend="$2"

  (
    export DRAGONGUI_PROFILE="${DRAGONGUI_PROFILE:-pi}"
    export DRAGONGUI_WGPU_BACKEND="$backend"
    export DRAGONGUI_WINDOW_BACKEND="$window_backend"
    export DRAGONGUI_SMOKE_FRAMES=1
    export DRAGONGUI_LOG=debug
    export RUST_BACKTRACE=1
    apply_window_backend_env
    log "DragonGUI one-frame probe: wgpu=$backend window=$window_backend"
    python - <<'PY'
import dragongui as dg

app = dg.App(title="DragonGUI GPU Diagnostic")
win = dg.Window("DragonGUI GPU Diagnostic", width=360, height=220)
with win:
    dg.Label("GPU diagnostic")
print(app.run(win))
PY
  ) || true
}

run_diag() {
  prepare_usb_dirs
  if [[ -d .venv ]]; then
    activate_venv
    copy_native_extension_if_available
  else
    printf 'Python virtual environment is missing; skipping DragonGUI import probes.\n'
  fi

  log "System"
  printf 'date: %s\n' "$(date -Is 2>/dev/null || date)"
  printf 'repo: %s\n' "$repo_root"
  printf 'usb_root: %s\n' "$usb_root"
  uname -a || true
  python3 --version || true
  if [[ -n "${VIRTUAL_ENV:-}" ]]; then
    python --version || true
  fi

  log "Display and DragonGUI environment"
  env | sort | grep -E '^(DISPLAY|WAYLAND|XDG_SESSION|XDG_CURRENT_DESKTOP|DESKTOP_SESSION|DRAGONGUI|WGPU|MESA|LIBGL|VK_|RUST_BACKTRACE)=' || true

  if [[ -n "${XDG_SESSION_ID:-}" ]] && command -v loginctl >/dev/null 2>&1; then
    optional_command "loginctl session" loginctl show-session "$XDG_SESSION_ID" -p Type -p Desktop -p Display -p Remote
  fi

  if [[ -n "${VIRTUAL_ENV:-}" ]]; then
    print_backend_info
  fi

  optional_command "Vulkan summary" vulkaninfo --summary
  optional_command "OpenGL renderer" glxinfo -B
  optional_shell "EGL info first 180 lines" "command -v eglinfo >/dev/null 2>&1 && eglinfo 2>&1 | sed -n '1,180p' || echo 'missing command: eglinfo'"
  optional_shell "GPU library resolution" "ldconfig -p 2>/dev/null | grep -E 'libEGL|libGLES|libGLX|libGL\\.so|libvulkan' || true"

  if [[ -n "${VIRTUAL_ENV:-}" ]]; then
    run_gpu_probe gl x11
    run_gpu_probe vulkan wayland
    run_gpu_probe vulkan auto
  fi
}

build_install_copy() {
  ensure_rust
  ensure_venv
  build_wheel
  install_wheel
  copy_native_extension
  verify_backend
}

run_smoke() {
  local target="${1:-examples/all_features_v3_demo.py}"
  [[ -f "$target" ]] || die "Python example not found: $target"
  activate_venv
  apply_window_backend_env
  export DRAGONGUI_SMOKE_FRAMES="${DRAGONGUI_SMOKE_FRAMES:-3}"
  log "Running smoke test: $target"
  python "$target"
}

run_demo() {
  local target="${1:-examples/all_features_v3_demo.py}"
  [[ -f "$target" ]] || die "Python example not found: $target"
  activate_venv
  apply_window_backend_env
  log "Running demo: $target"
  python "$target"
}

command_name="${1:-build-smoke}"
if [[ $# -gt 0 ]]; then
  shift
fi
target="${1:-examples/all_features_v3_demo.py}"

case "$command_name" in
  -h|--help|help)
    usage
    ;;
  deps)
    install_deps
    ;;
  setup)
    ensure_rust
    ensure_venv
    ;;
  build)
    build_install_copy
    ;;
  diag)
    run_diag
    ;;
  smoke)
    activate_venv
    copy_native_extension
    verify_backend
    run_smoke "$target"
    ;;
  run)
    activate_venv
    copy_native_extension
    verify_backend
    run_demo "$target"
    ;;
  build-smoke)
    build_install_copy
    run_smoke "$target"
    ;;
  build-run)
    build_install_copy
    run_demo "$target"
    ;;
  full)
    install_deps
    build_install_copy
    run_smoke "$target"
    ;;
  *)
    usage
    die "Unknown command: $command_name"
    ;;
esac
