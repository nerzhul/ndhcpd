#!/usr/bin/env bash
# Clang-based cross-linker wrapper for x86_64-unknown-freebsd.
#
# Used by .cargo/config.toml as the linker for the FreeBSD target.
# Requires clang (tested with clang 14+) and a FreeBSD sysroot.
#
# The sysroot is resolved in order:
#   1. $FREEBSD_SYSROOT environment variable
#   2. <workspace-root>/target/freebsd-sysroot  (default after running
#      scripts/fetch-freebsd-sysroot.sh)
#
# To prepare the sysroot:
#   ./scripts/fetch-freebsd-sysroot.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEFAULT_SYSROOT="${WORKSPACE_ROOT}/.freebsd-sysroot"
SYSROOT="${FREEBSD_SYSROOT:-${DEFAULT_SYSROOT}}"

if [[ ! -d "${SYSROOT}" ]]; then
    echo "error: FreeBSD sysroot not found at '${SYSROOT}'" >&2
    echo "       Run: ./scripts/fetch-freebsd-sysroot.sh" >&2
    echo "       Or set FREEBSD_SYSROOT to an existing sysroot directory." >&2
    exit 1
fi

exec clang \
    --target=x86_64-unknown-freebsd14 \
    --sysroot="${SYSROOT}" \
    "$@"
