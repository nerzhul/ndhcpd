#!/usr/bin/env bash
# Download and extract a minimal FreeBSD 14 sysroot for cross-compilation.
#
# Usage:
#   ./scripts/fetch-freebsd-sysroot.sh [SYSROOT_DIR]
#
# SYSROOT_DIR defaults to ./.freebsd-sysroot (outside target/ so cargo clean
# does not delete it). Can also be overridden via the FREEBSD_SYSROOT env var.
#
# The sysroot is populated from the official FreeBSD release base.txz.
# Only the directories needed for linking are extracted (lib/, usr/lib/,
# usr/include/), keeping the download size manageable (~60 MB).

set -euo pipefail

FREEBSD_VERSION="${FREEBSD_VERSION:-14.3-RELEASE}"
FREEBSD_ARCH="${FREEBSD_ARCH:-amd64}"
BASE_URL="https://download.freebsd.org/releases/${FREEBSD_ARCH}/${FREEBSD_VERSION}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SYSROOT_DIR="${1:-${FREEBSD_SYSROOT:-${WORKSPACE_ROOT}/.freebsd-sysroot}}"

echo "FreeBSD sysroot target: ${SYSROOT_DIR}"
echo "FreeBSD version       : ${FREEBSD_VERSION}"

mkdir -p "${SYSROOT_DIR}"

fetch_and_extract() {
    local archive="$1"
    shift
    local url="${BASE_URL}/${archive}"
    local tmp
    tmp="$(mktemp -d)"

    echo "Downloading ${url} ..."
    curl -fsSL --progress-bar -o "${tmp}/${archive}" "${url}"

    echo "Extracting ${archive} to ${SYSROOT_DIR} ..."
    # base.txz uses pax/tar format; extract into SYSROOT_DIR
    tar -xf "${tmp}/${archive}" -C "${SYSROOT_DIR}" "$@"
    rm -rf "${tmp}"
}

# Extract only the subset needed for linking
fetch_and_extract base.txz \
    "./lib" \
    "./usr/lib" \
    "./usr/lib32" \
    "./usr/include"

echo ""
echo "Sysroot ready at: ${SYSROOT_DIR}"
echo ""
echo "Set FREEBSD_SYSROOT=${SYSROOT_DIR} before running cargo, or rely on the"
echo "default path (.freebsd-sysroot) used by the linker wrapper."
