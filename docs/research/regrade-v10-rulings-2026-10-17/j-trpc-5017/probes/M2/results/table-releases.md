| Probe | base | head | v10.43.4 | v10.43.6 | v10.45.2 |
| --- | --- | --- | --- | --- | --- |
| `m2-01-helper.ts` | compiles | compiles | compiles | compiles | compiles |
| `m2-02-concat-never-input.ts` | **TS18048** | **TS18048** | **TS18048** | **TS18048** | **TS18048** |
| `m2-03-standalone-never-input.ts` | **TS18048, TS2345** | **TS18048, TS2345** | **TS18048, TS2345** | **TS2345** | **TS2345** |
| `m2-04-throwing-middleware-ctx.ts` | compiles | **TS2339** | compiles | compiles | compiles |
| `m2-05-never-ctx.ts` | **TS2339** | **TS2339** | **TS2339** | **TS2339** | **TS2339** |
| `m2-06-control-no-never.ts` | compiles | compiles | compiles | compiles | compiles |
| `m2-07-throwing-middleware-params.ts` | compiles | compiles | compiles | compiles | compiles |
| `m2-08-never-input-then-middleware.ts` | compiles | compiles | compiles | compiles | compiles |
| `m2-09-documented-ways.ts` | compiles | compiles | compiles | compiles | compiles |
