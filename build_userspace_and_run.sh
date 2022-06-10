#!/bin/bash

set -ex
cd `dirname $0`
cd userspace
#cargo build --release 
cd ..
userspace/place_all_binaries.sh 64 riscv64gc
cd kernel
cargo run
cd ..