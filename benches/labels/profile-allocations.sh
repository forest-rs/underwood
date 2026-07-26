#!/bin/sh
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -eu

binary=${1:-target/release/underwood_label_benchmark}
profile_rounds=${2:-1}
profile_labels=${3:-1}
profile_set=${4:-all}

case "$profile_set" in
all | capabilities | typing) ;;
*)
    echo "unknown profile set: $profile_set (expected all, capabilities, or typing)" >&2
    exit 1
    ;;
esac

if [ ! -x "$binary" ]; then
    echo "benchmark binary is missing: $binary" >&2
    echo "build it with: cargo build --release -p underwood_label_benchmark" >&2
    exit 1
fi

run_trace() {
    scenario=$1
    code=$2
    rounds=${3:-$profile_rounds}
    labels=${4:-$profile_labels}

    env MallocStackLogging=full UNDERWOOD_PROFILE_HOLD_SECS=3600 \
        UNDERWOOD_PROFILE_QUIET=1 \
        "$binary" "$code" "$rounds" "$labels" >/dev/null 2>&1 &
    pid=$!

    sleep 0.2
    if ! kill -0 "$pid" 2>/dev/null; then
        echo "benchmark exited before the profiler could attach: $scenario" >&2
        exit 1
    fi

    trace=$(mktemp "${TMPDIR:-/tmp}/underwood-malloc-history.XXXXXX")
    if ! malloc_history "$pid" -quiet -allEvents >"$trace"; then
        rm -f "$trace"
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
        echo "malloc_history failed: $scenario" >&2
        exit 1
    fi
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
        ' "$trace"
    rm -f "$trace"

    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
}

is_capability_scenario() {
    case "$1" in
    setup-identical | primed-identical | primed-mixed-display | \
        primed-mixed-editable | primed-editable-block | cold-identical | \
        cold-accessible | cold-link | cold-selectable | cold-editable | \
        upgrade-accessible | upgrade-link | upgrade-selectable | \
        upgrade-editable | editable-to-display | mixed-upgrade | \
        mixed-repeat | mixed-typing | editable-typing)
        return 0
        ;;
    *)
        return 1
        ;;
    esac
}

is_typing_scenario() {
    case "$1" in
    primed-mixed-editable | mixed-repeat | mixed-typing)
        return 0
        ;;
    *)
        return 1
        ;;
    esac
}

results=$(mktemp "${TMPDIR:-/tmp}/underwood-allocation-results.XXXXXX")
trap 'rm -f "$results"' EXIT HUP INT TERM

while read -r scenario code rounds labels; do
    if [ "$profile_set" = capabilities ] &&
        ! is_capability_scenario "$scenario"; then
        continue
    fi
    if [ "$profile_set" = typing ] &&
        ! is_typing_scenario "$scenario"; then
        continue
    fi
    run_trace "$scenario" "$code" "${rounds:-$profile_rounds}" \
        "${labels:-$profile_labels}" >>"$results"
done <<'SCENARIOS'
setup-identical s0
setup-identity s1
setup-cross-identical x0
cross-identical x1
setup-cross-distinct x2
cross-distinct x3
setup-shared-hit y0 1 2
shared-hit y1 1 2
primed-identical p0
primed-paint p1
primed-paint-slot p5
primed-unique p2
primed-region p3
primed-adjustment p4
primed-hit-query p6
primed-mixed-display m0
primed-mixed-editable m2
primed-editable-block b0
cold-identical c0
cold-accessible i3
cold-link i4
cold-selectable i0
cold-editable i1
upgrade-accessible u0
upgrade-link u1
upgrade-selectable u2
upgrade-editable u3
editable-to-display u4
mixed-upgrade m1
mixed-repeat m3
mixed-typing m4
editable-typing b1
retained-identical r0
retained-adjustment r1
paint-change a0
paint-slot-churn a3
alignment-churn a1
justification-churn a2
localized-edit e0
hit-query i2
width-churn w0
region-ready g0
region-churn g1
identity-churn h0
projection-identity-setup q0
projection-identity q1
projection-collapse-setup q2
projection-collapse q3
projection-expansion-setup q4
projection-expansion q5
SCENARIOS

awk -v profile_set="$profile_set" '
    BEGIN {
        OFS = "\t"
        print "scenario", "allocation_calls", "allocated_bytes"
    }
    {
        calls[$1] = $2
        bytes[$1] = $3
    }
    END {
        if (profile_set == "typing") {
            print "mixed-repeat", \
                calls["mixed-repeat"] - calls["primed-mixed-editable"], \
                bytes["mixed-repeat"] - bytes["primed-mixed-editable"]
            print "mixed-typing", \
                calls["mixed-typing"] - calls["primed-mixed-editable"], \
                bytes["mixed-typing"] - bytes["primed-mixed-editable"]
            exit
        }
        print "cold-identical", \
            calls["cold-identical"] - calls["setup-identical"], \
            bytes["cold-identical"] - bytes["setup-identical"]
        print "cold-accessible", \
            calls["cold-accessible"] - calls["setup-identical"], \
            bytes["cold-accessible"] - bytes["setup-identical"]
        print "cold-link", \
            calls["cold-link"] - calls["setup-identical"], \
            bytes["cold-link"] - bytes["setup-identical"]
        print "cold-selectable", \
            calls["cold-selectable"] - calls["setup-identical"], \
            bytes["cold-selectable"] - bytes["setup-identical"]
        print "cold-editable", \
            calls["cold-editable"] - calls["setup-identical"], \
            bytes["cold-editable"] - bytes["setup-identical"]
        print "upgrade-accessible", \
            calls["upgrade-accessible"] - calls["primed-identical"], \
            bytes["upgrade-accessible"] - bytes["primed-identical"]
        print "upgrade-link", \
            calls["upgrade-link"] - calls["primed-identical"], \
            bytes["upgrade-link"] - bytes["primed-identical"]
        print "upgrade-selectable", \
            calls["upgrade-selectable"] - calls["primed-identical"], \
            bytes["upgrade-selectable"] - bytes["primed-identical"]
        print "upgrade-editable", \
            calls["upgrade-editable"] - calls["primed-identical"], \
            bytes["upgrade-editable"] - bytes["primed-identical"]
        print "editable-to-display", \
            calls["editable-to-display"] - calls["primed-editable-block"], \
            bytes["editable-to-display"] - bytes["primed-editable-block"]
        print "mixed-upgrade", \
            calls["mixed-upgrade"] - calls["primed-mixed-display"], \
            bytes["mixed-upgrade"] - bytes["primed-mixed-display"]
        print "mixed-repeat", \
            calls["mixed-repeat"] - calls["primed-mixed-editable"], \
            bytes["mixed-repeat"] - bytes["primed-mixed-editable"]
        print "mixed-typing", \
            calls["mixed-typing"] - calls["primed-mixed-editable"], \
            bytes["mixed-typing"] - bytes["primed-mixed-editable"]
        print "editable-typing", \
            calls["editable-typing"] - calls["primed-editable-block"], \
            bytes["editable-typing"] - bytes["primed-editable-block"]
        if (profile_set == "capabilities") {
            exit
        }
        print "cross-identical", \
            calls["cross-identical"] - calls["setup-cross-identical"], \
            bytes["cross-identical"] - bytes["setup-cross-identical"]
        print "cross-distinct", \
            calls["cross-distinct"] - calls["setup-cross-distinct"], \
            bytes["cross-distinct"] - bytes["setup-cross-distinct"]
        print "shared-hit", \
            calls["shared-hit"] - calls["setup-shared-hit"], \
            bytes["shared-hit"] - bytes["setup-shared-hit"]
        print "retained-identical", \
            calls["retained-identical"] - calls["primed-identical"], \
            bytes["retained-identical"] - bytes["primed-identical"]
        print "retained-adjustment", \
            calls["retained-adjustment"] - calls["primed-adjustment"], \
            bytes["retained-adjustment"] - bytes["primed-adjustment"]
        print "paint-change", \
            calls["paint-change"] - calls["primed-paint"], \
            bytes["paint-change"] - bytes["primed-paint"]
        print "paint-slot-churn", \
            calls["paint-slot-churn"] - calls["primed-paint-slot"], \
            bytes["paint-slot-churn"] - bytes["primed-paint-slot"]
        print "alignment-churn", \
            calls["alignment-churn"] - calls["primed-adjustment"], \
            bytes["alignment-churn"] - bytes["primed-adjustment"]
        print "justification-churn", \
            calls["justification-churn"] - calls["primed-adjustment"], \
            bytes["justification-churn"] - bytes["primed-adjustment"]
        print "localized-edit", \
            calls["localized-edit"] - calls["primed-identical"], \
            bytes["localized-edit"] - bytes["primed-identical"]
        print "hit-query", \
            calls["hit-query"] - calls["primed-hit-query"], \
            bytes["hit-query"] - bytes["primed-hit-query"]
        print "width-churn", \
            calls["width-churn"] - calls["primed-unique"], \
            bytes["width-churn"] - bytes["primed-unique"]
        print "region-ready", \
            calls["region-ready"] - calls["primed-unique"], \
            bytes["region-ready"] - bytes["primed-unique"]
        print "region-churn", \
            calls["region-churn"] - calls["primed-region"], \
            bytes["region-churn"] - bytes["primed-region"]
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
