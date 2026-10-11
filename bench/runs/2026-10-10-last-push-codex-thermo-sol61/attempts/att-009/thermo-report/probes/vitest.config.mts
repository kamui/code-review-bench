const clone = '/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-009/clone';
export default {
  test: { environment: 'jsdom', include: ['*.test.tsx'] },
  resolve: {
    alias: [
      { find: /^@base-ui\/react\/(.*)$/, replacement: `${clone}/packages/react/src/$1/index.ts` },
      { find: /^@base-ui\/utils\/(.*)$/, replacement: `${clone}/packages/utils/src/$1` },
    ],
  },
};
