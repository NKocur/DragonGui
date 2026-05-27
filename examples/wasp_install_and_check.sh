#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

if [[ ! -d .venv ]]; then
  python3 -m venv .venv
fi

. .venv/bin/activate

WHEEL=""
if [[ -d ./py39 ]]; then
  WHEEL="$(find ./py39 -maxdepth 1 -name 'dragongui-*-cp39-abi3-*.whl' -print -quit)"
fi
if [[ -z "${WHEEL}" ]]; then
  WHEEL="$(find . -maxdepth 1 -name 'dragongui-*-cp39-abi3-*.whl' -print -quit)"
fi
if [[ -z "${WHEEL}" ]]; then
  echo "ERROR: Python 3.9-compatible DragonGUI wheel not found in ./py39 or current directory" >&2
  exit 1
fi

python -m pip install --force-reinstall "${WHEEL}"
python wasp_check_dragongui.py
