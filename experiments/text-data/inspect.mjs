// Copyright 2026 the Underwood Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

import fs from "node:fs";
import path from "node:path";

const outputDirectory = process.argv[2];
if (outputDirectory === undefined) {
  throw new Error("usage: node inspect.mjs OUTPUT_DIRECTORY");
}

const artifacts = ["empty", "minimal", "complex-segmentation"];
const observations = artifacts.map((artifact) => {
  const bytes = fs.readFileSync(path.join(outputDirectory, `${artifact}.wasm`));
  const module = new WebAssembly.Module(bytes);
  const imports = WebAssembly.Module.imports(module);
  if (imports.length !== 0) {
    throw new Error(`${artifact}.wasm unexpectedly imports host functions`);
  }
  const instance = new WebAssembly.Instance(module, {});
  const initialBytes = instance.exports.memory.buffer.byteLength;
  const dataEnd = Number(instance.exports.__data_end.value);
  const heapBase = Number(instance.exports.__heap_base.value);
  instance.exports.main();
  const warmBytes = instance.exports.memory.buffer.byteLength;
  return { artifact, initialBytes, dataEnd, heapBase, warmBytes };
});

const empty = observations[0];
console.log(
  [
    "artifact",
    "initial_bytes",
    "data_end",
    "heap_base",
    "warm_bytes",
    "incremental_initial",
    "incremental_warm",
  ].join("\t"),
);
for (const observation of observations) {
  console.log(
    [
      observation.artifact,
      observation.initialBytes,
      observation.dataEnd,
      observation.heapBase,
      observation.warmBytes,
      observation.initialBytes - empty.initialBytes,
      observation.warmBytes - empty.warmBytes,
    ].join("\t"),
  );
}
