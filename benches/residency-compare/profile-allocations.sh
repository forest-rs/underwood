#!/bin/sh
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

binary=${1:-target/release/underwood_residency_compare}

if [ "$(uname -s)" != Darwin ]; then
    echo "allocation profiling requires macOS malloc_history" >&2
    exit 1
fi
if [ ! -x "$binary" ]; then
    echo "comparison binary is missing: $binary" >&2
    echo "build it with: cargo build --release -p underwood_residency_compare" >&2
    exit 1
fi

results=$(mktemp "${TMPDIR:-/tmp}/underwood-residency-allocations.XXXXXX")
trap 'rm -f "$results"' EXIT HUP INT TERM

run_trace() {
    scenario=$1
    scale=$2
    rounds=$3
    temporary=$(mktemp -d "${TMPDIR:-/tmp}/underwood-residency-trace.XXXXXX")

    env MallocStackLogging=full RESIDENCY_PROFILE_HOLD_SECS=3600 \
        "$binary" "$scenario" "$scale" "$rounds" >"$temporary/stdout" \
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

    malloc_history "$pid" -quiet -allEvents >"$temporary/history"
    awk -v scenario="$scenario" -v scale="$scale" '
        /^(ALLOC|CALLOC|REALLOC)/ {
            fields = split($0, size_start, "\\[size=")
            if (fields > 1) {
                split(size_start[2], size_end, "]")
                calls += 1
                bytes += size_end[1]
            }
        }
        END {
            printf "%s:%s\t%d\t%d\n", scenario, scale, calls, bytes
        }
    ' "$temporary/history" >>"$results"

    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    rm -rf "$temporary"
}

run_trace runtime-baseline 1 1
run_trace underwood-font-baseline 1 1
run_trace parley-font-baseline 1 1

for scale in 64 1000; do
    run_trace underwood-label-display "$scale" 1
    run_trace underwood-label-editable "$scale" 1
    run_trace underwood-label-editable-warm "$scale" 1
    run_trace parley-label "$scale" 1
    run_trace underwood-repeat "$scale" 1
    run_trace parley-repeat "$scale" 1
    run_trace underwood-edit "$scale" 1
    run_trace underwood-edit-warm "$scale" 1
    run_trace parley-edit "$scale" 1
    run_trace underwood-hit-setup "$scale" 1
    run_trace underwood-hit-exact "$scale" 1
    run_trace underwood-hit-closest "$scale" 1
    run_trace underwood-position "$scale" 1
    run_trace parley-hit-setup "$scale" 1
    run_trace parley-hit-exact "$scale" 1
    run_trace parley-hit-closest "$scale" 1
    run_trace parley-position "$scale" 1
    run_trace underwood-churn "$scale" 1
    run_trace parley-churn "$scale" 1
done

awk '
    BEGIN {
        OFS = "\t"
        print "scenario", "scale", "allocation_calls", "allocated_bytes"
    }
    {
        calls[$1] = $2
        bytes[$1] = $3
    }
    function emit(name, scale, target, baseline) {
        print name, scale, calls[target] - calls[baseline], bytes[target] - bytes[baseline]
    }
    END {
        emit("underwood-fonts", 1, "underwood-font-baseline:1", "runtime-baseline:1")
        emit("parley-fonts", 1, "parley-font-baseline:1", "runtime-baseline:1")
        for (scale_index = 1; scale_index <= 2; scale_index++) {
            scale = scale_index == 1 ? 64 : 1000
            suffix = ":" scale
            emit("underwood-label-display", scale,
                "underwood-label-display" suffix, "underwood-font-baseline:1")
            emit("underwood-label-editable", scale,
                "underwood-label-editable" suffix, "underwood-font-baseline:1")
            emit("underwood-label-editable-warm", scale,
                "underwood-label-editable-warm" suffix, "underwood-font-baseline:1")
            emit("parley-label", scale,
                "parley-label" suffix, "parley-font-baseline:1")
            emit("underwood-repeat", scale,
                "underwood-repeat" suffix, "underwood-label-editable" suffix)
            emit("parley-repeat", scale,
                "parley-repeat" suffix, "parley-label" suffix)
            emit("underwood-edit", scale,
                "underwood-edit" suffix, "underwood-label-editable" suffix)
            emit("underwood-edit-warm", scale,
                "underwood-edit-warm" suffix, "underwood-label-editable-warm" suffix)
            emit("parley-edit", scale,
                "parley-edit" suffix, "parley-label" suffix)
            emit("underwood-hit-exact", scale,
                "underwood-hit-exact" suffix, "underwood-hit-setup" suffix)
            emit("underwood-hit-closest", scale,
                "underwood-hit-closest" suffix, "underwood-hit-setup" suffix)
            emit("underwood-position", scale,
                "underwood-position" suffix, "underwood-hit-setup" suffix)
            emit("parley-hit-exact", scale,
                "parley-hit-exact" suffix, "parley-hit-setup" suffix)
            emit("parley-hit-closest", scale,
                "parley-hit-closest" suffix, "parley-hit-setup" suffix)
            emit("parley-position", scale,
                "parley-position" suffix, "parley-hit-setup" suffix)
            emit("underwood-churn", scale,
                "underwood-churn" suffix, "underwood-font-baseline:1")
            emit("parley-churn", scale,
                "parley-churn" suffix, "parley-font-baseline:1")
        }
    }
' "$results"
