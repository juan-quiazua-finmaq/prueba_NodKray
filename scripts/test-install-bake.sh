#!/usr/bin/env bash
# The installer must download from the hardcoded repo with no NODKRAY_REPO prompt.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"

for script in "${root}/scripts/install.sh" "${root}/scripts/install.ps1"; do
  if ! grep -q 'juan-quiazua-finmaq/prueba_NodKray' "${script}"; then
    echo "${script} must hardcode juan-quiazua-finmaq/prueba_NodKray" >&2
    exit 1
  fi
  if grep -q 'Set NODKRAY_REPO' "${script}"; then
    echo "${script} must not ask for NODKRAY_REPO" >&2
    exit 1
  fi
  if grep -q '__BAKE_REPO__' "${script}"; then
    echo "${script} must not use a bake placeholder" >&2
    exit 1
  fi
done

echo "install scripts use the hardcoded repo"
