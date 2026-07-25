#!/bin/sh
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

binary=${1:-target/release/underwood_semantic_scene_benchmark}
paragraphs=${2:-1000}

if [ ! -x "$binary" ]; then
    echo "benchmark binary is missing: $binary" >&2
    echo "build it with: cargo build --release -p underwood_semantic_scene_benchmark" >&2
    exit 1
fi

run_trace() {
    scenario=$1
    code=$2
    raw=$(mktemp "${TMPDIR:-/tmp}/underwood-semantic-allocation-trace.XXXXXX")
    ready=$(mktemp "${TMPDIR:-/tmp}/underwood-semantic-profile-ready.XXXXXX")
    rm -f "$ready"

    env MallocStackLogging=full UNDERWOOD_PROFILE_HOLD_SECS=3600 \
        UNDERWOOD_PROFILE_READY_FILE="$ready" \
        UNDERWOOD_PROFILE_QUIET=1 \
        "$binary" "$code" "$paragraphs" >/dev/null 2>&1 &
    pid=$!

    attempts=0
    while [ ! -f "$ready" ] && [ "$attempts" -lt 600 ]; do
        attempts=$((attempts + 1))
        if ! kill -0 "$pid" 2>/dev/null; then
            rm -f "$raw" "$ready"
            echo "benchmark exited before the profiler could attach: $scenario" >&2
            exit 1
        fi
        sleep 0.05
    done

    if [ ! -f "$ready" ]; then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        rm -f "$raw" "$ready"
        echo "benchmark did not become profiler-ready after $attempts attempts: $scenario" >&2
        exit 1
    fi

    malloc_history "$pid" -quiet -allEvents >"$raw"

    awk -v scenario="$scenario" '
        /^(ALLOC|CALLOC|REALLOC)/ {
            fields = split($0, size_start, "\\[size=")
            if (fields > 1) {
                split(size_start[2], size_end, "]")
                calls += 1
                bytes += size_end[1]
            }
        }
        END {
            printf "%s\t%d\t%d\n", scenario, calls, bytes
        }
    ' "$raw"

    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    rm -f "$raw" "$ready"
}

results=$(mktemp "${TMPDIR:-/tmp}/underwood-semantic-allocation-results.XXXXXX")
trap 'rm -f "$results"' EXIT HUP INT TERM

while read -r scenario code; do
    run_trace "$scenario" "$code" >>"$results"
done <<'SCENARIOS'
setup-retained s0
retained r0
setup-edit s1
edit-staging d0
localized-prepare p0
localized-edit e0
SCENARIOS

awk -v paragraphs="$paragraphs" '
    BEGIN {
        OFS = "\t"
        print "scenario", "paragraphs", "allocation_calls", "allocated_bytes"
    }
    {
        calls[$1] = $2
        bytes[$1] = $3
    }
    END {
        print "retained-unchanged", paragraphs, \
            calls["retained"] - calls["setup-retained"], \
            bytes["retained"] - bytes["setup-retained"]
        print "edit-staging", paragraphs, \
            calls["edit-staging"] - calls["setup-edit"], \
            bytes["edit-staging"] - bytes["setup-edit"]
        print "localized-prepare", paragraphs, \
            calls["localized-prepare"] - calls["setup-edit"], \
            bytes["localized-prepare"] - bytes["setup-edit"]
        print "localized-edit-total", paragraphs, \
            calls["localized-edit"] - calls["setup-edit"], \
            bytes["localized-edit"] - bytes["setup-edit"]
    }
' "$results"
