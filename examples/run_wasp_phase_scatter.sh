#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

APP_DIR="$(pwd)"
VENV_DIR="${WASP_VENV_DIR:-$HOME/.cache/wasp-phase-scatter-venv}"
echo "Using local venv: ${VENV_DIR}"

if [[ ! -d "${VENV_DIR}" ]]; then
  python3 -m venv "${VENV_DIR}"
fi

. "${VENV_DIR}/bin/activate"
python -m pip install --upgrade pip
python -m pip install numpy

WHEEL="${WASP_WHEEL:-}"
if [[ -z "${WHEEL}" ]]; then
  WHEEL="$(
    APP_DIR="$APP_DIR" python - <<'PY'
import os
from pathlib import Path

from pip._vendor.packaging import tags
from pip._vendor.packaging.utils import InvalidWheelFilename, parse_wheel_filename

app_dir = Path(os.environ["APP_DIR"])
supported = set(tags.sys_tags())
candidates = sorted(app_dir.glob("dragongui-*.whl"), reverse=True)
compatible = []

for path in candidates:
    try:
        _name, _version, _build, wheel_tags = parse_wheel_filename(path.name)
    except InvalidWheelFilename:
        continue
    if wheel_tags & supported:
        compatible.append(path)

if compatible:
    print(compatible[0])
PY
  )"
fi
if [[ -z "${WHEEL}" ]]; then
  echo "ERROR: no compatible DragonGUI wheel found next to this launcher." >&2
  echo "Available wheels:" >&2
  find "$APP_DIR" -maxdepth 1 -name 'dragongui-*.whl' -print >&2
  exit 1
fi

echo "Using DragonGUI wheel: ${WHEEL}"
python -m pip install --force-reinstall --no-deps "${WHEEL}"

export DRAGONGUI_PROFILE="${DRAGONGUI_PROFILE:-pi}"
export DRAGONGUI_WGPU_BACKEND="${DRAGONGUI_WGPU_BACKEND:-gl}"
export DRAGONGUI_WINDOW_BACKEND="${DRAGONGUI_WINDOW_BACKEND:-x11}"

python rpi_v4l2_phase_scatter.py "$@"
