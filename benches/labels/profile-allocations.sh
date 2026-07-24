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

run_trace() {
    scenario=$1
    code=$2

    env MallocStackLogging=full UNDERWOOD_PROFILE_HOLD_SECS=3600 \
        UNDERWOOD_PROFILE_QUIET=1 \
        "$binary" "$code" 1 1 >/dev/null 2>&1 &
    pid=$!

    sleep 0.2
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "benchmark exited before the profiler could attach: $scenario" >&2
        exit 1
    fi

    malloc_history "$pid" -quiet -allEvents |
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
        '

    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
}

results=$(mktemp "${TMPDIR:-/tmp}/underwood-allocation-results.XXXXXX")
trap 'rm -f "$results"' EXIT HUP INT TERM

while read -r scenario code; do
    run_trace "$scenario" "$code" >>"$results"
done <<'SCENARIOS'
setup-identical s0
setup-identity s1
primed-identical p0
primed-paint p1
primed-unique p2
cold-identical c0
retained-identical r0
paint-change a0
localized-edit e0
interaction-materialization i0
width-churn w0
region-ready g0
identity-churn h0
projection-identity-setup q0
projection-identity q1
projection-collapse-setup q2
projection-collapse q3
projection-expansion-setup q4
projection-expansion q5
SCENARIOS

awk '
    BEGIN {
        OFS = "\t"
        print "scenario", "allocation_calls", "allocated_bytes"
    }
    {
        calls[$1] = $2
        bytes[$1] = $3
    }
    END {
        print "cold-identical", \
            calls["cold-identical"] - calls["setup-identical"], \
            bytes["cold-identical"] - bytes["setup-identical"]
        print "retained-identical", \
            calls["retained-identical"] - calls["primed-identical"], \
            bytes["retained-identical"] - bytes["primed-identical"]
        print "paint-change", \
            calls["paint-change"] - calls["primed-paint"], \
            bytes["paint-change"] - bytes["primed-paint"]
        print "localized-edit", \
            calls["localized-edit"] - calls["primed-identical"], \
            bytes["localized-edit"] - bytes["primed-identical"]
        print "interaction-materialization", \
            calls["interaction-materialization"] - calls["setup-identical"], \
            bytes["interaction-materialization"] - bytes["setup-identical"]
        print "width-churn", \
            calls["width-churn"] - calls["primed-unique"], \
            bytes["width-churn"] - bytes["primed-unique"]
        print "region-ready", \
            calls["region-ready"] - calls["primed-unique"], \
            bytes["region-ready"] - bytes["primed-unique"]
        print "identity-churn", \
            calls["identity-churn"] - calls["setup-identity"], \
            bytes["identity-churn"] - bytes["setup-identity"]
        print "projection-identity", \
            calls["projection-identity"] - calls["projection-identity-setup"], \
            bytes["projection-identity"] - bytes["projection-identity-setup"]
        print "projection-collapse", \
            calls["projection-collapse"] - calls["projection-collapse-setup"], \
            bytes["projection-collapse"] - bytes["projection-collapse-setup"]
        print "projection-expansion", \
            calls["projection-expansion"] - calls["projection-expansion-setup"], \
            bytes["projection-expansion"] - bytes["projection-expansion-setup"]
    }
' "$results"
