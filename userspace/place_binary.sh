#!/bin/bash

set -e

export DEST=$4
export CRATE=$3
export ARCH=$2
export BITS=$1
export FILE=`dirname $0`/target/$ARCH-unknown-none-elf/release/$CRATE

cp $FILE `dirname $0`/../drive-loopback/$DEST
echo Copied successfully

if mountpoint `dirname $0`/../drive-loopback; then
    : # ... things which should happen if command's result code was 0
	sync `dirname $0`/../drive-loopback/$DEST
else
	echo "Error: Run \`sudo mount -o loop drive.img drive-loopback\` to mount the drive image first"
    exit 1
fi