#!/bin/bash

set -exou pipefail

PROJ=/home/zjp/KMiri

cd $PROJ/kmiri
./miri install --debug

# cd $PROJ/tests/init
# OSDK_LOCAL_DEV=1 cargo osdk miri test

TOCK_BOARD=$PROJ/tock/boards/qemu_rv64_virt
cd $TOCK_BOARD
MIRIFLAGS="-Zkmiri-toml=$TOCK_BOARD/kmiri.toml" MIRI_SYSROOT="$(rustc --print sysroot)" cargo miri run --target riscv64imac-unknown-none-elf
