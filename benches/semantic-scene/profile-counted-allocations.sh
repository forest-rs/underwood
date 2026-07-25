#!/bin/sh
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

binary=${1:-target/release/underwood_semantic_scene_benchmark}

if [ ! -x "$binary" ]; then
    echo "allocation-counting benchmark binary is missing: $binary" >&2
    echo "build it with: cargo build --release -p underwood_semantic_scene_benchmark --features allocation-counting" >&2
    exit 1
fi

for paragraphs in 64 1000; do
    "$binary" retained "$paragraphs"
    "$binary" edit-staging "$paragraphs"
    "$binary" localized-prepare "$paragraphs"
    "$binary" localized-edit "$paragraphs"
done
