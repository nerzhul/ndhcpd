# Cross-Compilation Guide

This document explains how to build `home-router` for targets other than the
host platform, with a focus on **FreeBSD x86_64** from a Linux host.

## Supported targets

| Target | Host | Toolchain required |
|---|---|---|
| `x86_64-unknown-linux-gnu` | Linux x86_64 | native `cargo` |
| `x86_64-unknown-freebsd` | Linux x86_64 | clang ≥ 14, llvm-ar, FreeBSD sysroot |

---

## Linux → Linux (native)

No special setup required:

```bash
cargo build --release
# or
make release
```

---

## Linux → FreeBSD x86_64

### Prerequisites

| Tool | Minimum version | Install (Debian/Ubuntu) |
|---|---|---|
| Rust target | — | `rustup target add x86_64-unknown-freebsd` |
| clang | 14 | `apt install clang` |
| llvm-ar | 14 | `apt install llvm` |
| curl + tar | any | usually pre-installed |

The cross-linker wrapper ([scripts/freebsd-linker.sh](scripts/freebsd-linker.sh))
and the C cross-compiler wrapper ([scripts/freebsd-cc.sh](scripts/freebsd-cc.sh))
use `clang --target=x86_64-unknown-freebsd14 --sysroot=...` under the hood.
They are wired into Cargo via [.cargo/config.toml](.cargo/config.toml).

### Step 1 — Add the Rust target

```bash
rustup target add x86_64-unknown-freebsd
```

### Step 2 — Download the FreeBSD sysroot (once)

The sysroot is a subset of the official FreeBSD 14 base system
(`lib/`, `usr/lib/`, `usr/include/`). It is fetched from
`download.freebsd.org` and stored in `.freebsd-sysroot/` at the
workspace root (excluded from git and **not** inside `target/` so
`cargo clean` does not delete it).

```bash
make freebsd-sysroot
```

This downloads roughly **60 MB** and takes about a minute depending on
your connection speed. Run it **once**; subsequent builds reuse the
cached sysroot.

You can override the sysroot location via the `FREEBSD_SYSROOT`
environment variable:

```bash
FREEBSD_SYSROOT=/opt/freebsd14-sysroot make freebsd-sysroot
FREEBSD_SYSROOT=/opt/freebsd14-sysroot make freebsd-release
```

### Step 3 — Build

```bash
# debug
make freebsd

# release (recommended for deployment)
make freebsd-release
```

Binaries land in:

```
target/x86_64-unknown-freebsd/release/dhcp-server
target/x86_64-unknown-freebsd/release/dhcp-cli
```

### How it works

```
cargo build --target x86_64-unknown-freebsd
       │
       ├── Rust crates → rustc cross-compiles to FreeBSD target
       │
       ├── C build scripts (e.g. libsqlite3-sys, ring)
       │       └── CC = scripts/freebsd-cc.sh
       │               └── clang --target=x86_64-unknown-freebsd14
       │                         --sysroot=.freebsd-sysroot
       │
       └── Linker
               └── scripts/freebsd-linker.sh
                       └── clang --target=x86_64-unknown-freebsd14
                                 --sysroot=.freebsd-sysroot
```

The FreeBSD sysroot provides the platform libraries normally absent on
a Linux host (`libexecinfo`, `libkvm`, `libprocstat`, `libdevstat`, …)
that are pulled in transitively by Rust's standard library for the
FreeBSD target.

### Troubleshooting

#### `FreeBSD sysroot not found`

The wrapper scripts could not find `.freebsd-sysroot/`.

```bash
make freebsd-sysroot          # re-run the sysroot download
# — or —
FREEBSD_SYSROOT=/path/to/sysroot make freebsd-release
```

#### `cargo clean` wiped the sysroot

`cargo clean` only removes the `target/` directory, so the sysroot in
`.freebsd-sysroot/` survives. If you previously had it inside `target/`
(from an older setup), just re-run `make freebsd-sysroot`.

#### clang not found / wrong version

```bash
clang --version   # must be ≥ 14
```

Install via your package manager or, on NixOS, add `clang` to your
shell environment.

#### Linker errors about `__errno_location`, `stat64`, etc.

These are glibc symbols — they appear when C code (e.g. sqlite3.c) is
compiled by the host Linux compiler instead of the FreeBSD-targeting
wrapper. Make sure `.cargo/config.toml` is present and intact, then
clean and rebuild:

```bash
cargo clean --target x86_64-unknown-freebsd
make freebsd-release
```
