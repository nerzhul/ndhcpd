#!/usr/bin/env bash
# Clang-based cross C-compiler wrapper for x86_64-unknown-freebsd.
#
# Used by .cargo/config.toml (CC_x86_64_unknown_freebsd) so that build
# scripts (e.g. libsqlite3-sys) compile C code for FreeBSD, not Linux.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEFAULT_SYSROOT="${WORKSPACE_ROOT}/.freebsd-sysroot"
SYSROOT="${FREEBSD_SYSROOT:-${DEFAULT_SYSROOT}}"

if [[ ! -d "${SYSROOT}" ]]; then
    echo "error: FreeBSD sysroot not found at '${SYSROOT}'" >&2
    echo "       Run: ./scripts/fetch-freebsd-sysroot.sh" >&2
    exit 1
fi

exec clang \
    --target=x86_64-unknown-freebsd14 \
    --sysroot="${SYSROOT}" \
    "$@"
