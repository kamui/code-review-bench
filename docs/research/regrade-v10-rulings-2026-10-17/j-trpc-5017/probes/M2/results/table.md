| Probe | base | head | m2-never-check | gt-j1-revert-second-gate | both-revert-second-gate-and-never-check |
| --- | --- | --- | --- | --- | --- |
| `m2-01-helper.ts` | compiles | compiles | **TS2322** | compiles | **TS2322** |
| `m2-02-concat-never-input.ts` | **TS18048** | **TS18048** | compiles | **TS18048** | compiles |
| `m2-03-standalone-never-input.ts` | **TS18048, TS2345** | **TS18048, TS2345** | **TS2345** | **TS18048, TS2345** | **TS2345** |
| `m2-04-throwing-middleware-ctx.ts` | compiles | **TS2339** | **TS2339** | compiles | compiles |
| `m2-05-never-ctx.ts` | **TS2339** | **TS2339** | compiles | **TS2339** | compiles |
| `m2-06-control-no-never.ts` | compiles | compiles | compiles | compiles | compiles |
| `m2-07-throwing-middleware-params.ts` | compiles | compiles | compiles | compiles | compiles |
| `m2-08-never-input-then-middleware.ts` | compiles | compiles | compiles | compiles | compiles |
| `m2-09-documented-ways.ts` | compiles | compiles | compiles | compiles | compiles |
