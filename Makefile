PROJ := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/..)
SYSROOT := $(shell rustc --print sysroot)
MIRI_SYSROOT_REBUILT := /root/.cache/miri

.PHONY: install
install:
	[ -d "$(MIRI_SYSROOT_REBUILT)" ] && MIRI_SYSROOT="$(MIRI_SYSROOT_REBUILT)" || MIRI_SYSROOT="$(SYSROOT)"; \
	export MIRI_SYSROOT; \
	cd $(PROJ)/kmiri && ./miri install --debug && \
	cd $(PROJ)/kmiri-helper && cargo install --path .

.PHONY: install-osdk
install-osdk:
	cd $(PROJ)/asterinas && OSDK_LOCAL_DEV=1 make install_osdk

.PHONY: asterinas
ASTERINAS_TEST := init
asterinas: install install-osdk
	cd $(PROJ)/tests/$(ASTERINAS_TEST) && \
		OSDK_LOCAL_DEV=1 cargo osdk miri test

TOCK_BOARD := $(PROJ)/tock/boards/qemu_rv64_virt
.PHONY: tock
tock: install
	cd $(TOCK_BOARD) && \
		MIRIFLAGS="-Zkmiri-toml=$(TOCK_BOARD)/kmiri.toml" \
		MIRI_SYSROOT="$(SYSROOT)" \
		cargo miri run --target riscv64imac-unknown-none-elf

MIRI_TEST_NAME := debugger_test
MIRI_TEST_FILE := tests/pass/$(MIRI_TEST_NAME).rs
__KMIRI_DIR_TARGET := $(PROJ)/kmiri/target
ANALYSIS_DIR := $(__KMIRI_DIR_TARGET)/analysis
.PHONY: debugger
debugger: install
	export __KMIRI_DIR_TARGET=$(__KMIRI_DIR_TARGET) && \
	export LD_LIBRARY_PATH=$(SYSROOT)/lib && \
    trap 'rm -f $(MIRI_TEST_NAME)' EXIT && \
	rm -rf $(ANALYSIS_DIR) && \
	kmiri-helper $(MIRI_TEST_FILE) --emit=metadata && \
	mv $(ANALYSIS_DIR)/*.json $(__KMIRI_DIR_TARGET)/analysis.json && \
	MIRIFLAGS="$(MIRIFLAGS) --debugger" ./miri run $(MIRI_TEST_FILE)

# Run Miri's ui test suite (pass + fail), e.g. `make test-fail` runs all
# `physical-copy*` tests: pass cases under tests/pass and fail cases under
# tests/fail (in-file `//~` annotations + `.stderr` reference files).
# Add `BLESS=1` to regenerate the `.stderr` files.
MIRI_TEST_FILTER ?= physical-copy
BLESS ?=
.PHONY: test
test: install
	./miri test $(if $(BLESS),--bless) $(MIRI_TEST_FILTER)

.PHONY: kmiri-setup
# This generate a precompiled sysroot in `/root/.cache/miri`.
kmiri-setup: install
	export LD_LIBRARY_PATH=$(SYSROOT)/lib/rustlib/x86_64-unknown-linux-gnu/lib && \
	which cargo-miri && \
    cargo miri setup

.PHONY: test-in-blueos
test-in-blueos: kmiri-setup
	export __KMIRI_DIR_TARGET=$(__KMIRI_DIR_TARGET) && \
	export MIRI_SYSROOT="$(MIRI_SYSROOT_REBUILT)" && \
	export LD_LIBRARY_PATH=$(SYSROOT)/lib/rustlib/x86_64-unknown-linux-gnu/lib && \
    trap 'rm -f $(MIRI_TEST_NAME)' EXIT && \
	rm -rf $(ANALYSIS_DIR) && \
	kmiri-helper $(MIRI_TEST) && \
	mv $(ANALYSIS_DIR)/*.json $(__KMIRI_DIR_TARGET)/analysis.json && \
	MIRIFLAGS=--debugger ./miri run $(MIRI_TEST)
