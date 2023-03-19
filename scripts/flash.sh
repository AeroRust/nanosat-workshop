#!/usr/bin/env bash

set -e

BUILD_MODE=""
case "$1" in
"" | "release")
    bash scripts/build.sh
    BUILD_MODE="release"
    ;;
"debug")
    bash scripts/build.sh debug
    BUILD_MODE="debug"
    ;;
*)
    echo "Wrong argument. Only \"debug\"/\"release\" arguments are supported"
    exit 1
    ;;
esac

export ESP_ARCH=riscv32imac-unknown-none-elf

web-flash --chip esp32c3 target/${ESP_ARCH}/${BUILD_MODE}/nanosat_workshop
