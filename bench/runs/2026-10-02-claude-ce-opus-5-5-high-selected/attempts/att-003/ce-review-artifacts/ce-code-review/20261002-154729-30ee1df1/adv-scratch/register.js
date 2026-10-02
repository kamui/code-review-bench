'use strict';
process.env.BABEL_DISABLE_CACHE = '1';
require('/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone/node_modules/@babel/register')({
  extensions: ['.ts'],
  cwd: '/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone',
  babelrc: false,
  configFile: false,
  only: ['/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone/src', '/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone-work/ce-review-artifacts/ce-code-review/20261002-154729-30ee1df1/adv-scratch'],
  plugins: ['/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone/node_modules/@babel/plugin-transform-typescript'],
  presets: [['/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone/node_modules/@babel/preset-env', { bugfixes: true, targets: { node: 'current' } }]],
  cache: false,
});
