#!/bin/bash

set -e

echo $0

export ARCH=$2
export BITS=$1
export BIN=`dirname $0`/place_binary.sh

#$BIN $BITS $ARCH test_program other_prog
#$BIN $BITS $ARCH shell_program main
