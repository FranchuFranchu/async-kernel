#!/bin/bash

set -ex
cd `dirname $0`
cd userspace
#cargo build --release 
cd ..
userspace/place_all_binaries.sh 64 riscv64gc

cd kernel/kernel_main
cargo build
cd ../..

./link.sh 64 riscv64gc-unknown-none-elf

cd kernel/kernel_bootloader
cargo build
cd ../..

./run.sh 64 riscv64gc-unknown-none-elf kernel/target/riscv64gc-unknown-none-elf/debug/kernel_bootloader

