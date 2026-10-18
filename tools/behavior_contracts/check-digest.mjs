import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

function canonical(value) {
  if (value === null) return Buffer.from('n');
  if (typeof value === 'boolean') return Buffer.from(value ? 't' : 'f');
  if (typeof value === 'bigint') return Buffer.from(`i${value};`);
  if (typeof value === 'number') {
    assert(Number.isFinite(value));
    const raw = Buffer.alloc(9);
    raw[0] = 'd'.charCodeAt(0);
    raw.writeDoubleBE(value, 1);
    return raw;
  }
  if (typeof value === 'string') {
    const raw = Buffer.from(value, 'utf8');
    return Buffer.concat([Buffer.from(`s${raw.length}:`), raw]);
  }
  if (Array.isArray(value)) {
    return Buffer.concat([Buffer.from(`[${value.length}:`), ...value.map(canonical)]);
  }
  const keys = Object.keys(value).sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  return Buffer.concat([Buffer.from(`{${keys.length}:`),
    ...keys.flatMap(key => [canonical(key), canonical(value[key])])]);
}

const values = [null, true, 1n, 1.0, 4e-5, 9007199254740993n, -0.0, '雪😀',
  { '雪': [true, null, 10.0], a: 1n }];
const vectors = JSON.parse(readFileSync(new URL('./digest-vectors.json', import.meta.url), 'utf8'));
assert.equal(vectors.length, values.length);
for (const [index, vector] of vectors.entries()) {
  const bytes = canonical(values[index]);
  assert.equal(bytes.toString('hex'), vector.canonical_hex, vector.name);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), vector.sha256, vector.name);
}
console.log(`${vectors.length} canonical digest vectors passed`);
