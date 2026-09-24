#!/usr/bin/env bash
# Prove the release bake cannot rewrite the abort check into the real repo.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
baked="$(mktemp)"
trap 'rm -f "${baked}"' EXIT

sed "s|__BAKE_REPO__|acme/NodKray|g" "${root}/scripts/install.sh" > "${baked}"

if grep -q '__BAKE_REPO__' "${baked}"; then
  echo "bake left leftover __BAKE_REPO__" >&2
  exit 1
fi

if grep -Eq 'REPO == "acme/NodKray"' "${baked}"; then
  echo "bake rewrote the abort check to the real repo" >&2
  exit 1
fi

# Simulate the published default (baked repo, no NODKRAY_REPO).
REPO="acme/NodKray"
if [[ "${REPO}" == "__BAKE_REPO__" || "${REPO}" == "__REPO__" || "${REPO}" == "OWNER/NodKray" ]]; then
  echo "baked default incorrectly treated as unset" >&2
  exit 1
fi

# Setting NODKRAY_REPO to the real repo must also stay valid.
REPO="${NODKRAY_REPO:-acme/NodKray}"
NODKRAY_REPO="acme/NodKray"
REPO="${NODKRAY_REPO:-__BAKE_REPO__}"
if [[ "${REPO}" == "__BAKE_REPO__" || "${REPO}" == "__REPO__" || "${REPO}" == "OWNER/NodKray" ]]; then
  echo "NODKRAY_REPO=acme/NodKray incorrectly treated as unset" >&2
  exit 1
fi

# An unbaked source script still aborts without NODKRAY_REPO.
unbaked_default="__BAKE_REPO__"
if [[ "${unbaked_default}" != "__BAKE_REPO__" ]]; then
  echo "unbaked default must remain a placeholder" >&2
  exit 1
fi

echo "install.sh bake contract ok"
