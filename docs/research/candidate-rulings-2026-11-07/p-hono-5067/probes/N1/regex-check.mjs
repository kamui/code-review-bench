// The head's jsonRegex and fix-json's, against the Content-Type values in the
// head's own JSON tests (src/validator/validator.test.ts) and this group's headers.
const head = /^application\/([a-z-\.]+\+)?json(;\s*[a-zA-Z0-9\-]+\=([^;]+))*$/i
const fix = /^application\/([a-z-\.]+\+)?json(\s*;\s*[a-zA-Z0-9\-]+\=([^;]+))*$/i
const values = [
  ['application/json', true],
  ['Application/JSON', true],
  ['text/plain;charset=utf-8', false],
  ['application/merge-patch+json', true],
  ['application/vnd.api+json', true],
  ['Xapplication/json', false],
  ['application/json ; charset=utf-8', null],
  ['application/jsonx', false],
  ['application/json ;', false],
]
for (const [v, expected] of values) {
  console.log(`${JSON.stringify(v).padEnd(36)} head=${head.test(v)} fix=${fix.test(v)} test-expects=${expected ?? 'n/a'}`)
}
