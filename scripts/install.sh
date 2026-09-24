#!/usr/bin/env bash
# Download the NodKray binary built by GitHub Actions (latest Release).
set -euo pipefail

# Release workflow replaces __BAKE_REPO__ with github.repository.
# The abort check uses different tokens so a baked script never treats the
# real repo as "unset".
REPO="${NODKRAY_REPO:-__BAKE_REPO__}"
VERSION="${NODKRAY_VERSION:-latest}"
PREFIX="${NODKRAY_PREFIX:-${HOME}/.local}"
BIN_DIR="${PREFIX}/bin"

usage() {
  cat <<'EOF'
Usage: install.sh [--uninstall]

  (default)   Download and install the nodkray binary.
  --uninstall Remove only the nodkray binary this script installed.

Environment:
  NODKRAY_REPO      owner/name (only needed for an unbaked source script)
  NODKRAY_VERSION   release tag, or "latest"
  NODKRAY_PREFIX    install prefix (default: ~/.local)
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "${REPO}" == "__BAKE_REPO__" || "${REPO}" == "__REPO__" || "${REPO}" == "OWNER/NodKray" ]]; then
  if [[ -n "${GITHUB_REPOSITORY:-}" ]]; then
    REPO="${GITHUB_REPOSITORY}"
  else
    echo "Set NODKRAY_REPO=owner/name (example: NODKRAY_REPO=acme/NodKray)" >&2
    exit 2
  fi
fi

if [[ "${1:-}" == "--uninstall" ]]; then
  target="${BIN_DIR}/nodkray"
  if [[ -e "${target}" ]]; then
    rm -f "${target}"
    echo "Removed ${target}"
  else
    echo "No nodkray binary at ${target}"
  fi
  echo "Left untouched: MCP servers, Spec-Kit, Herdr, agent configs, and project files."
  echo "For NodKray config and memory: nodkray uninstall"
  echo "For project overlay files:     nodkray uninstall --project"
  exit 0
fi

os="$(uname -s)"
arch="$(uname -m)"
case "${os}" in
  Linux) os_tag="unknown-linux-gnu" ;;
  Darwin) os_tag="apple-darwin" ;;
  MINGW*|MSYS*|CYGWIN*)
    echo "On Windows use install.ps1 or download the .zip from GitHub Releases." >&2
    exit 2
    ;;
  *)
    echo "Unsupported OS: ${os}" >&2
    exit 2
    ;;
esac

case "${arch}" in
  x86_64|amd64) arch_tag="x86_64" ;;
  arm64|aarch64) arch_tag="aarch64" ;;
  *)
    echo "Unsupported architecture: ${arch}" >&2
    exit 2
    ;;
esac

target="${arch_tag}-${os_tag}"
asset="nodkray-${target}.tar.gz"

if [[ "${VERSION}" == "latest" ]]; then
  url="https://github.com/${REPO}/releases/latest/download/${asset}"
else
  url="https://github.com/${REPO}/releases/download/${VERSION}/${asset}"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

echo "Downloading ${url}"
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "${url}" -o "${tmp}/${asset}"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "${tmp}/${asset}" "${url}"
else
  echo "Need curl or wget." >&2
  exit 4
fi

tar -C "${tmp}" -xzf "${tmp}/${asset}"
if [[ ! -f "${tmp}/nodkray" ]]; then
  echo "Archive did not contain nodkray." >&2
  exit 1
fi

mkdir -p "${BIN_DIR}"
install -m 0755 "${tmp}/nodkray" "${BIN_DIR}/nodkray"
echo "Installed ${BIN_DIR}/nodkray"
"${BIN_DIR}/nodkray" --version || true

case ":${PATH}:" in
  *":${BIN_DIR}:"*) ;;
  *)
    echo
    echo "Add this to your shell profile:"
    echo "  export PATH=\"${BIN_DIR}:\$PATH\""
    ;;
esac
