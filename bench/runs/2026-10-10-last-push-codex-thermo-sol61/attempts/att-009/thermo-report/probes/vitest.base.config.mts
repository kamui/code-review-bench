import config from './vitest.config.mts';
const clone = '/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-009/clone';
const baseline = new URL('./base-FieldControl.tsx', import.meta.url).pathname;
export default {
  ...config,
  plugins: [{
    name: 'use-pinned-baseline-control',
    enforce: 'pre',
    resolveId(source, importer) {
      if (source === './control/FieldControl' && importer === `${clone}/packages/react/src/field/index.parts.ts`) {
        return baseline;
      }
    },
  }],
};
