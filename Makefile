PROJ := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/..)
SYSROOT := $(shell rustc --print sysroot)
MIRI_SYSROOT_REBUILT := /root/.cache/miri

ASTERINAS_TOML := $(PROJ)/tests/asterinas.toml
ASTERINAS_ENV_VARS := OSDK_LOCAL_DEV=1 MIRIFLAGS="-Zkmiri-toml=$(ASTERINAS_TOML) -Zmiri-permissive-provenance"
# A test kernel may bring its own KMiri configuration in tests/<name>/kmiri.toml.
asterinas_test_toml = $(or $(wildcard $(PROJ)/tests/$(1)/kmiri.toml),$(ASTERINAS_TOML))

define RUN_IRQ_DEADLOCK_TEST
log=$$(mktemp); \
if (cd $(PROJ)/tests/irq-deadlock && $(ASTERINAS_ENV_VARS) cargo osdk miri test) >"$$log" 2>&1; then \
	cat "$$log"; rm -f "$$log"; echo "irq-deadlock unexpectedly completed without aborting"; exit 1; \
fi; \
cat "$$log"; \
if grep -F "IRQ watchdog: possible self-deadlock" "$$log" >/dev/null; then \
	echo "Observed expected IRQ watchdog deadlock diagnostic."; rm -f "$$log"; \
else \
	rm -f "$$log"; echo "irq-deadlock failed without the expected watchdog diagnostic"; exit 1; \
fi
endef

define RUN_IRQ_RCU_UAF_TEST
log=$$(mktemp); \
if (cd $(PROJ)/tests/irq-rcu-uaf && $(ASTERINAS_ENV_VARS) cargo osdk miri test) >"$$log" 2>&1; then \
	cat "$$log"; rm -f "$$log"; echo "irq-rcu-uaf unexpectedly completed without aborting"; exit 1; \
fi; \
cat "$$log"; \
if awk '/Undefined Behavior: / && /dangling reference \(use-after-free\)/ { found = 1 } END { exit !found }' "$$log"; then \
	echo "Observed expected RCU use-after-free diagnostic."; rm -f "$$log"; \
else \
	rm -f "$$log"; echo "irq-rcu-uaf failed without the expected use-after-free diagnostic"; exit 1; \
fi
endef

DEBUG := --debug

.PHONY: install
install:
	[ -d "$(MIRI_SYSROOT_REBUILT)" ] && MIRI_SYSROOT="$(MIRI_SYSROOT_REBUILT)" || MIRI_SYSROOT="$(SYSROOT)"; \
	export MIRI_SYSROOT; \
	cd $(PROJ)/kmiri && ./miri install $(DEBUG) && \
	cd $(PROJ)/kmiri-helper && cargo install --path .

.PHONY: install-osdk
install-osdk:
	cd $(PROJ)/asterinas && OSDK_LOCAL_DEV=1 make install_osdk

.PHONY: asterinas
ASTERINAS_TEST := init
ifeq ($(ASTERINAS_TEST),irq-deadlock)
asterinas: install install-osdk
	$(RUN_IRQ_DEADLOCK_TEST)

else ifeq ($(ASTERINAS_TEST),irq-rcu-uaf)
asterinas: install install-osdk
	$(RUN_IRQ_RCU_UAF_TEST)

else
asterinas: install install-osdk
	echo $(ASTERINAS_TEST)
	cd $(PROJ)/tests/$(ASTERINAS_TEST) && OSDK_LOCAL_DEV=1 \
		MIRIFLAGS="-Zkmiri-toml=$(call asterinas_test_toml,$(ASTERINAS_TEST)) -Zmiri-permissive-provenance" \
		cargo osdk miri test
endif

ASTERINAS_TESTS := init frame-realloc page-table-realloc segment-zeroed \
                   task-switch-back task-exit-last task-exit-with-runnable \
                   irq-simulation irq-trap-path irq-deadlock irq-rcu-uaf \
                   irq-rcu-uaf-lifo heap-box-drop heap-large-object \
                   heap-slot-end-pointer tsc-read dyn-cpu-local
.PHONY: asterinas-test-all
asterinas-test-all: install install-osdk
	set -eu; \
	for test in $(ASTERINAS_TESTS); do \
		echo "Running test: $${test}"; \
		if [ "$${test}" = irq-deadlock ]; then \
			$(RUN_IRQ_DEADLOCK_TEST); \
		elif [ "$${test}" = irq-rcu-uaf ]; then \
			$(RUN_IRQ_RCU_UAF_TEST); \
		else \
			toml=$(PROJ)/tests/$${test}/kmiri.toml; [ -f "$$toml" ] || toml=$(ASTERINAS_TOML); \
			(cd $(PROJ)/tests/$${test} && OSDK_LOCAL_DEV=1 \
				MIRIFLAGS="-Zkmiri-toml=$$toml -Zmiri-permissive-provenance" cargo osdk miri test); \
		fi; \
	done

# Runs the full Asterinas kernel (`asterinas/kernel`, the real `#[ostd::main]`
# entry). KMiri has no user mode yet, so the kernel is expected to boot until
# it looks for an init process to start, and to stop there.
ASTERINAS_KERNEL_TOML := $(PROJ)/tests/asterinas-kernel.toml
ASTERINAS_KERNEL_STOP := neither an initramfs init nor a usable root filesystem was available
.PHONY: asterinas-kernel
asterinas-kernel: install install-osdk
	log=$$(mktemp); \
	(cd $(PROJ)/asterinas/kernel && OSDK_LOCAL_DEV=1 VDSO_LIBRARY_DIR=$(PROJ)/env/linux_vdso \
		MIRIFLAGS="-Zkmiri-toml=$(ASTERINAS_KERNEL_TOML) -Zmiri-permissive-provenance" \
		cargo osdk miri) >"$$log" 2>&1; \
	cat "$$log"; \
	if grep -F "$(ASTERINAS_KERNEL_STOP)" "$$log" >/dev/null; then \
		echo "The kernel reached the expected stop: there is no init process to run."; rm -f "$$log"; \
	else \
		rm -f "$$log"; echo "the kernel stopped before it looked for an init process"; exit 1; \
	fi

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

# This filter can be a substring in the path, e.g. a prefix `no-paging`
# to run all pass and fail tests. The default filter is for all kmiri tests.
MIRI_TEST_FILTER ?= kmiri
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
