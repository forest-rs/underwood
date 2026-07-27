#!/bin/sh
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

binary=${1:-target/release/underwood_label_benchmark}
rounds=${2:-1000}

if [ ! -x "$binary" ]; then
    echo "benchmark binary is missing: $binary" >&2
    echo "build it with: cargo build --release -p underwood_label_benchmark" >&2
    exit 1
fi

for units in 64 1024 8192; do
    "$binary" interaction-hit-closest "$rounds" "$units"
    "$binary" interaction-position-at "$rounds" "$units"
done
