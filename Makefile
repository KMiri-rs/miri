PROJ := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/..)

.PHONY: install asterinas tock test

install:
	cd $(PROJ)/kmiri && ./miri install --debug && \
	cd $(PROJ)/kmiri-helper && cargo install --path .

asterinas: install
	cd $(PROJ)/tests/init && \
		OSDK_LOCAL_DEV=1 cargo osdk miri test

TOCK_BOARD := $(PROJ)/tock/boards/qemu_rv64_virt
tock: install
	cd $(TOCK_BOARD) && \
		MIRIFLAGS="-Zkmiri-toml=$(TOCK_BOARD)/kmiri.toml" \
		MIRI_SYSROOT="$$(rustc --print sysroot)" \
		cargo miri run --target riscv64imac-unknown-none-elf

MIRI_TEST_NAME := debugger_test
MIRI_TEST := tests/pass/$(MIRI_TEST_NAME).rs
__KMIRI_DIR_TARGET := $(PROJ)/kmiri/target
ANALYSIS_DIR := $(__KMIRI_DIR_TARGET)/analysis
test: install
	export __KMIRI_DIR_TARGET=$(__KMIRI_DIR_TARGET) && \
	export LD_LIBRARY_PATH=$$(rustc --print=sysroot)/lib && \
    trap 'rm -f $(MIRI_TEST_NAME)' EXIT && \
	rm -rf $(ANALYSIS_DIR) && \
	kmiri-helper $(MIRI_TEST) && \
	mv $(ANALYSIS_DIR)/*.json $(__KMIRI_DIR_TARGET)/analysis.json && \
	MIRIFLAGS=--debugger ./miri run $(MIRI_TEST)
