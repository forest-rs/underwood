#!/bin/sh
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

binary=${1:-target/release/underwood_label_benchmark}

if [ ! -x "$binary" ]; then
    echo "benchmark binary is missing: $binary" >&2
    echo "build it with: cargo build --release -p underwood_label_benchmark" >&2
    exit 1
fi

for paragraphs in 64 1000 2048; do
    "$binary" mixed-upgrade 1 "$paragraphs"
    "$binary" mixed-repeat 1000 "$paragraphs"
    "$binary" mixed-typing 100 "$paragraphs"
done
