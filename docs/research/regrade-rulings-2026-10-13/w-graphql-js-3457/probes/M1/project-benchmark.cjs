'use strict';
// Runs one of the project's own benchmark modules against the package built at one checkout.
//
//   node project-benchmark.cjs <checkout> <label> <benchmark module> <out.json>
//
// The project's harness (benchmark/benchmark.js) installs each revision with npm into the system temporary
// directory. This runner instead resolves the module's `graphql/...` imports to <checkout>/npmDist and calls
// its `measure()` the way the harness does: `count` calls make one sample. Seven samples are taken after two
// warm-up samples.
const path = require('path');
const fs = require('fs');
const Module = require('module');

const [checkoutArg, label, benchmarkPath, outPath] = process.argv.slice(2);
const dist = path.join(path.resolve(checkoutArg), 'npmDist');
const resolve = Module._resolveFilename;
Module._resolveFilename = function (request, ...rest) {
  if (request.startsWith('graphql/')) {
    return resolve.call(this, path.join(dist, request.slice('graphql/'.length)), ...rest);
  }
  return resolve.call(this, request, ...rest);
};

const benchmark = require(path.resolve(benchmarkPath));
function sample() {
  const start = process.hrtime.bigint();
  for (let i = 0; i < benchmark.count; i++) {
    benchmark.measure();
  }
  return Number(process.hrtime.bigint() - start) / 1e6 / benchmark.count;
}
sample();
sample();
const samples = Array.from({ length: 7 }, sample).map((s) => Number(s.toFixed(3)));
const sorted = [...samples].sort((a, b) => a - b);
const row = {
  label,
  benchmark: benchmark.name,
  file: path.basename(benchmarkPath),
  count: benchmark.count,
  ms_per_measure_samples: samples,
  median_ms_per_measure: sorted[3],
};
console.log(JSON.stringify(row));
fs.writeFileSync(outPath, JSON.stringify({ node: process.version, ...row }, null, 1) + '\n');
