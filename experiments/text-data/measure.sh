#!/usr/bin/env bash
# Copyright 2026 the Underwood Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

root="$(git rev-parse --show-toplevel)"
package="underwood_text_data_experiment"
target="wasm32-unknown-unknown"
profile="wind-tunnel"
build_dir="${root}/target/${target}/${profile}"
output_dir="${root}/target/text-data-wind-tunnel"

mkdir -p "${output_dir}"

cargo build --locked --profile "${profile}" --target "${target}" \
  -p "${package}" --bin empty
cp "${build_dir}/empty.wasm" "${output_dir}/empty.wasm"

cargo build --locked --profile "${profile}" --target "${target}" \
  -p "${package}" --bin "${package}"
cp "${build_dir}/${package}.wasm" "${output_dir}/minimal.wasm"

cargo build --locked --profile "${profile}" --target "${target}" \
  -p "${package}" --bin "${package}" --features complex-scripts
cp "${build_dir}/${package}.wasm" "${output_dir}/complex-segmentation.wasm"

for artifact in empty minimal complex-segmentation; do
  brotli --quality=11 --force \
    --output="${output_dir}/${artifact}.wasm.br" \
    "${output_dir}/${artifact}.wasm"
done

empty_raw="$(wc -c <"${output_dir}/empty.wasm" | tr -d ' ')"
empty_brotli="$(wc -c <"${output_dir}/empty.wasm.br" | tr -d ' ')"
report="${output_dir}/sizes.tsv"

{
  printf 'artifact\traw_bytes\tbrotli_bytes\tincremental_raw\tincremental_brotli\tsha256\n'
  for artifact in empty minimal complex-segmentation; do
    raw="$(wc -c <"${output_dir}/${artifact}.wasm" | tr -d ' ')"
    compressed="$(wc -c <"${output_dir}/${artifact}.wasm.br" | tr -d ' ')"
    digest="$(shasum -a 256 "${output_dir}/${artifact}.wasm" | cut -d ' ' -f 1)"
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
      "${artifact}" \
      "${raw}" \
      "${compressed}" \
      "$((raw - empty_raw))" \
      "$((compressed - empty_brotli))" \
      "${digest}"
  done
} >"${report}"

cat "${report}"

node "${root}/experiments/text-data/inspect.mjs" "${output_dir}" \
  >"${output_dir}/memory.tsv"
cat "${output_dir}/memory.tsv"

cargo run --quiet --locked --release -p "${package}" \
  >"${output_dir}/minimal-throughput.txt"
cargo run --quiet --locked --release -p "${package}" --features complex-scripts \
  >"${output_dir}/complex-segmentation-throughput.txt"
cat "${output_dir}/minimal-throughput.txt"
cat "${output_dir}/complex-segmentation-throughput.txt"
