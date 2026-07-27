#!/bin/sh
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

binary=${1:-target/release/underwood_residency_compare}
samples=${2:-3}

if [ "$(uname -s)" != Darwin ]; then
    echo "live memory profiling requires macOS heap and vmmap" >&2
    exit 1
fi
if [ ! -x "$binary" ]; then
    echo "comparison binary is missing: $binary" >&2
    echo "build it with: cargo build --release -p underwood_residency_compare" >&2
    exit 1
fi

run_observation() {
    scenario=$1
    scale=$2
    sample=$3
    temporary=$(mktemp -d "${TMPDIR:-/tmp}/underwood-residency.XXXXXX")

    env RESIDENCY_PROFILE_HOLD_SECS=3600 \
        "$binary" "$scenario" "$scale" 1 >"$temporary/stdout" \
        2>"$temporary/stderr" &
    pid=$!

    ready=0
    for _attempt in $(seq 1 200); do
        if rg -q '^profiler_ready' "$temporary/stdout"; then
            ready=1
            break
        fi
        if ! kill -0 "$pid" 2>/dev/null; then
            break
        fi
        sleep 0.05
    done
    if [ "$ready" -ne 1 ]; then
        cat "$temporary/stderr" >&2
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        rm -rf "$temporary"
        echo "comparison process did not become profiler-ready: $scenario/$scale" >&2
        exit 1
    fi

    /usr/bin/heap -q "$pid" >"$temporary/heap"
    /usr/bin/vmmap -summary "$pid" >"$temporary/vmmap"
    heap_nodes=$(
        awk '/^All zones: [0-9][0-9]* nodes [(]/ { print $3; exit }' \
            "$temporary/heap"
    )
    heap_bytes=$(
        awk '/^All zones: [0-9][0-9]* nodes [(]/ {
            value = $5
            gsub("[^0-9]", "", value)
            print value
            exit
        }' "$temporary/heap"
    )
    footprint=$(
        awk '/^Physical footprint:/ { print $3; exit }' "$temporary/vmmap"
    )
    peak=$(
        awk '$1 == "Physical" && $2 == "footprint" && $3 == "(peak):" {
            print $4
            exit
        }' \
            "$temporary/vmmap"
    )
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$scenario" "$scale" "$sample" "$heap_nodes" "$heap_bytes" "$footprint/$peak"

    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    rm -rf "$temporary"
}

printf 'scenario\tscale\tsample\tlive_heap_nodes\tlive_heap_bytes\tfootprint/peak\n'
for sample in $(seq 1 "$samples"); do
    run_observation runtime-baseline 1 "$sample"
    run_observation underwood-font-baseline 1 "$sample"
    run_observation parley-font-baseline 1 "$sample"
done

for scale in 64 1000; do
    for sample in $(seq 1 "$samples"); do
        for scenario in \
            underwood-label-display \
            underwood-label-editable \
            underwood-label-editable-warm \
            parley-label \
            underwood-document-display \
            underwood-document-mixed \
            underwood-document-mixed-warm \
            parley-document-paragraphs \
            parley-document-flat \
            underwood-churn \
            parley-churn
        do
            run_observation "$scenario" "$scale" "$sample"
        done
    done
done
