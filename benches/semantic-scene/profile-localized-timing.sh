#!/bin/sh
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

binary=${1:-target/release/underwood_semantic_scene_benchmark}
samples=${2:-21}

if [ ! -x "$binary" ]; then
    echo "semantic-scene benchmark binary is missing: $binary" >&2
    echo "build it with: cargo build --release -p underwood_semantic_scene_benchmark" >&2
    exit 1
fi

case $samples in
    ''|*[!0-9]*)
        echo "sample count must be an odd positive integer" >&2
        exit 1
        ;;
esac
if [ "$samples" -eq 0 ] || [ $((samples % 2)) -eq 0 ]; then
    echo "sample count must be an odd positive integer" >&2
    exit 1
fi

results=$(mktemp "${TMPDIR:-/tmp}/underwood-localized-timing.XXXXXX")
sorted=$(mktemp "${TMPDIR:-/tmp}/underwood-localized-timing-sorted.XXXXXX")
trap 'rm -f "$results" "$sorted"' EXIT HUP INT TERM

printf 'scenario\tparagraphs\tsamples\tmin_ns\tmedian_ns\tmax_ns\n'
for paragraphs in 64 1000; do
    : >"$results"
    sample=0
    while [ "$sample" -lt "$samples" ]; do
        "$binary" localized-prepare "$paragraphs" |
            awk -F '\t' '
                {
                    for (field = 1; field <= NF; field += 1) {
                        if ($field ~ /^total_ns=/) {
                            split($field, value, "=")
                            print value[2]
                        }
                    }
                }
            ' >>"$results"
        sample=$((sample + 1))
    done
    sort -n "$results" >"$sorted"
    awk -v paragraphs="$paragraphs" -v samples="$samples" '
        NR == 1 { minimum = $1 }
        NR == (samples + 1) / 2 { median = $1 }
        { maximum = $1 }
        END {
            printf "localized-prepare\t%d\t%d\t%d\t%d\t%d\n",
                paragraphs, samples, minimum, median, maximum
        }
    ' "$sorted"
done
