# Makefile for home-router
#
# Cross-compilation notes:
#   - Linux targets use plain `cargo` (native or with the rustup target installed).
#   - FreeBSD targets use clang as the cross-linker (see .cargo/config.toml) and
#     require a FreeBSD sysroot. Prepare it once with:
#       make freebsd-sysroot
#     Then build normally with `make freebsd` or `make freebsd-release`.
#     The sysroot path can be overridden via the FREEBSD_SYSROOT env variable.

CARGO        := cargo
BUILD_DIR    := target

# Targets
LINUX_TARGET   := x86_64-unknown-linux-gnu
FREEBSD_TARGET := x86_64-unknown-freebsd

.PHONY: all build release linux linux-release freebsd freebsd-release \
        freebsd-sysroot test clippy fmt fmt-check clean install-targets help

# Default: debug build for the host platform
all: build

## Build (debug) for the host platform
build:
	$(CARGO) build

## Build (release) for the host platform
release:
	$(CARGO) build --release

## Build (debug) targeting Linux x86_64
linux:
	$(CARGO) build --target $(LINUX_TARGET)

## Build (release) targeting Linux x86_64
linux-release:
	$(CARGO) build --release --target $(LINUX_TARGET)

## Build (debug) targeting FreeBSD x86_64
freebsd:
	$(CARGO) build --target $(FREEBSD_TARGET)

## Build (release) targeting FreeBSD x86_64
freebsd-release:
	$(CARGO) build --release --target $(FREEBSD_TARGET)

## Download and extract a FreeBSD 14 sysroot for cross-compilation (run once)
freebsd-sysroot:
	chmod +x scripts/fetch-freebsd-sysroot.sh scripts/freebsd-linker.sh scripts/freebsd-cc.sh
	./scripts/fetch-freebsd-sysroot.sh

## Run the test suite
test:
	$(CARGO) test

## Run Clippy lints
clippy:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

## Format the source code
fmt:
	$(CARGO) fmt --all

## Check formatting without modifying files
fmt-check:
	$(CARGO) fmt --all -- --check

## Install the required Rust rustup target for Linux cross-compilation
install-targets:
	rustup target add $(LINUX_TARGET)
	rustup target add $(FREEBSD_TARGET)

## Remove build artefacts
clean:
	$(CARGO) clean

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "  build            Debug build for the host platform (default)"
	@echo "  release          Release build for the host platform"
	@echo "  linux            Debug build for $(LINUX_TARGET)"
	@echo "  linux-release    Release build for $(LINUX_TARGET)"
	@echo "  freebsd-sysroot  Download FreeBSD 14 sysroot (required once before freebsd builds)"
	@echo "  freebsd          Debug build for $(FREEBSD_TARGET)"
	@echo "  freebsd-release  Release build for $(FREEBSD_TARGET)"
	@echo "  test             Run the test suite"
	@echo "  clippy           Run Clippy lints"
	@echo "  fmt              Format source code"
	@echo "  fmt-check        Check formatting"
	@echo "  install-targets  Install rustup Linux cross-compilation target"
	@echo "  clean            Remove build artefacts"
