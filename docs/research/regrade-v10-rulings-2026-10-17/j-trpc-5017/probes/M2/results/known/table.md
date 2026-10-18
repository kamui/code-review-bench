| Probe | base | head | m2-never-check | gt-j1-revert-second-gate | both-revert-second-gate-and-never-check |
| --- | --- | --- | --- | --- | --- |
| `GT-j1-generic-1-compose.ts` | compiles | **TS2345** | **TS2345** | compiles | compiles |
| `GT-j1-generic-2-read.ts` | compiles | **TS2339** | **TS2339** | compiles | compiles |
| `GT-j1-generic-3-inline.ts` | compiles | **TS2339** | **TS2339** | compiles | compiles |
| `GT-j1-nonobject-1-ctx-undefined.ts` | compiles | **TS18048** | **TS18048** | compiles | compiles |
| `GT-j1-nonobject-2-ctx-maybe.ts` | compiles | **TS18048** | **TS18048** | compiles | compiles |
| `GT-j1-nonobject-3-ctx-null.ts` | compiles | **TS18047** | **TS18047** | compiles | compiles |
| `GT-j1-nonobject-4-input-read.ts` | compiles | **TS18048** | **TS18048** | compiles | compiles |
| `GT-j1-nonobject-5-input-call.ts` | compiles | **TS2578** | **TS2578** | compiles | compiles |
| `GT-j1-nonobject-6-input-null.ts` | compiles | **TS18047** | **TS18047** | compiles | compiles |
| `GT-j1-nonobject-7-input-concat.ts` | compiles | **TS18048** | **TS18048** | compiles | compiles |
| `control-string-input.ts` | **TS2322, TS2345** | compiles | compiles | compiles | compiles |
