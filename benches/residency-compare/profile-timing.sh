#!/bin/sh
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

binary=${1:-target/release/underwood_residency_compare}
samples=${2:-21}

if [ ! -x "$binary" ]; then
    echo "comparison binary is missing: $binary" >&2
    echo "build it with: cargo build --release -p underwood_residency_compare" >&2
    exit 1
fi

case "$samples" in
'' | *[!0-9]* | 0)
    echo "samples must be a positive integer" >&2
    exit 1
    ;;
esac

run_timing() {
    output=$("$binary" "$@")
    printf '%s\n' "$output" | awk '/^timing/'
}

for scale in 64 1000; do
    if [ "$scale" -eq 64 ]; then
        repeat_rounds=1000
    else
        repeat_rounds=100
    fi
    for sample in $(seq 1 "$samples"); do
        run_timing underwood-repeat "$scale" "$repeat_rounds"
        run_timing parley-repeat "$scale" "$repeat_rounds"
        run_timing underwood-edit "$scale" 1000
        run_timing underwood-edit-warm "$scale" 1000
        run_timing parley-edit "$scale" 1000
        run_timing underwood-hit-exact "$scale" 10000
        run_timing parley-hit-exact "$scale" 10000
        run_timing underwood-hit-closest "$scale" 10000
        run_timing parley-hit-closest "$scale" 10000
        run_timing underwood-position "$scale" 10000
        run_timing parley-position "$scale" 10000
        run_timing underwood-churn "$scale" 1
        run_timing parley-churn "$scale" 1
        echo "sample_complete	scale=$scale	sample=$sample" >&2
    done
done
