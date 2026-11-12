#!/usr/bin/env bash
# Builds the Leos Rust core for iOS.
#
# The device and the simulator need separate builds, because the simulator
# runs on the host architecture.
#
#   ios/scripts/build-rust.sh device
#   ios/scripts/build-rust.sh simulator
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
MODE="${1:-device}"

case "$MODE" in
    device)
        TARGETS=(aarch64-apple-ios)
        ;;
    simulator)
        TARGETS=(aarch64-apple-ios-sim x86_64-apple-ios)
        ;;
    *)
        echo "usage: $0 {device|simulator}" >&2
        exit 1
        ;;
esac

for target in "${TARGETS[@]}"; do
    echo "building leos-ffi for $target"
    (cd "$ROOT" && cargo build --release --package leos-ffi --target "$target")
done

echo
echo "The static libraries are in:"
for target in "${TARGETS[@]}"; do
    echo "  $ROOT/target/$target/release/libleos_ffi.a"
done
echo
echo "Add the one that matches your build to the app target under"
echo "Build Phases > Link Binary With Libraries, and point the bridging"
echo "header at ios/Leos/Bridge/leos_ffi.h."
