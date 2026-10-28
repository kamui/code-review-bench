# j-trpc-5017 (trpc/trpc PR 5017, TypeScript monorepo)

Labels: **read** = read in a file at `review-head`; **reported** = from target.json / receipt / verdict text; **inferred** = my reasoning, not observed.

## 1. Toolchain as pinned

| Fact | Value | Label / source |
|---|---|---|
| Package manager | `pnpm@8.5.1` (`packageManager`), `pnpm-lock.yaml` lockfileVersion 6.0 | read: package.json, pnpm-lock.yaml |
| Node | engines `^18.17.0`; `.nvmrc` `^18.17`; `.tool-versions` `nodejs 18.16.0` (sandbox node is v24.21.0, so the pinned range is not met) | read: package.json, .nvmrc, .tool-versions |
| TypeScript | `^5.1.3`, locked `typescript@5.1.3` | read: package.json, pnpm-lock.yaml:22387 |
| Test runner | vitest `^0.32.0` (locked 0.32.0); `packages/tests` `test` script = `concurrently` of `vitest --run` and `tsc --noEmit --pretty` | read: packages/tests/package.json |
| Type tests | vitest `expectTypeOf` files; the repo's real type check is `tsc --noEmit` in `packages/tests` | read (verdicts say the same) |
| Key configs | root `tsconfig.json` (paths map `@trpc/*` to `packages/*/src`, `types: [node, vitest/globals]`, `strict`, `noUncheckedIndexedAccess`); `packages/tests/tsconfig.json` extends it, `include: [server, showcase, setupTests.ts, vitest.config.ts]`, `skipLibCheck`, no `incremental`/`composite` | read |
| `.npmrc` | `public-hoist-pattern[]=@types/*`, `zod`, `playwright-core`; `side-effects-cache=false` | read |
| Install | `pnpm install --frozen-lockfile --offline --store-dir <cache>/pnpm-store`; root `postinstall` = `pnpm codegen` (tsx, turbo, run-p); examples run a prisma postinstall | read (package.json) + reported (target.json) |
| Cache archive | `home/` (prisma engine cache), `corepack/` (pnpm 8.5.1), `pnpm-store/` (73k entries). Archive sha256 on disk `8f53201c...` (rebuilt 2026-10-22); receipt says `ddfc465a...`, target.json `9ea0916b...` (three different values, so the archive entry in `dependency_identity` is stale) | read (tar -tzf, sha256sum) |

## 2. Commands the graded claims needed

82 distinct claims in refusal rows (41 first, 41 second). About 71 (36 first, 35 second) wanted a compiler run; the others needed no run or needed the network.

| # | Check | argv from clone root (cwd `clone`) | Claims | Example |
|---|---|---|---|---|
| A | `tsc` on a scratch tsconfig/probe in `clone-work` (the dominant need): `Overwrite<A,B>` with unknown/undefined/any/never second operand, array/tuple input after `.use()`, union ctx, throw-only middleware inference, 576 to 1,024-pair equivalence matrices | `packages/tests/node_modules/.bin/tsc --noEmit -p ../clone-work/<scratch>.json`. Graders' own refused forms (read from verdicts): `./packages/tests/node_modules/.bin/tsc --noEmit -p ../clone-work/tsconfig-probe.json`, also `clone-work/eq/tsconfig.json`, `clone-work/tsconfig.never.json` | ~34 first, ~33 second | first `blind-1d04b7/1/c1`; second `blind-39d6b3/1/c1` |
| B | `tsc` on the repo's own tests tsconfig (baseline / sanity) | `packages/tests/node_modules/.bin/tsc --noEmit -p packages/tests/tsconfig.json` (target.json allowance form is `cd packages/tests && ./node_modules/.bin/tsc --noEmit -p tsconfig.json`; the `cd` is not expressible as argv with cwd `clone`) | not named in rows; it is the target.json smoke and allowance text | n/a |
| C | Fetch issue trpc/trpc#5020 text | none (network) | 1 first | `blind-1d04b7/8/c11` |
| D | No run needed; settled from `references.json` or notes reading | none | ~10 (e.g. `blind-989064/1/c1`, `blind-b753da/4/c7`, `blind-21a62f/3/c3`) | n/a |

Scratch layout seen in the verdicts: `clone-work/probe.ts`, `probe/probe.ts`, `eq/eq.ts`, `eq/tsconfig.json`, `overwrite-eq.ts`, `tsconfig.never.json`. No claim asked for vitest, pnpm scripts, turbo or eslint. target.json `unavailable` says the vitest suite "is not the discriminating check, only tsc is" (reported).

## 3. Can each run offline in the sandbox under current provisioning?

- A and B: yes for the binary and dependencies (reported: smoke `type-check` at "both" revisions ran in about 5 s; inferred: `typescript@5.1.3` is a root and `packages/tests` dependency so both `node_modules/.bin/tsc` exist). Only the node binary is needed at run time. The pnpm `.bin/tsc` is an `sh` shim that execs `node` with a relative `NODE_PATH` (inferred), so the sandbox needs `/bin/sh` (mounted) and `node` on `PATH`.
- No network at run time: tsc downloads nothing (inferred). `pnpm`, `npx`, `turbo` must stay out: pnpm 8.5.1 is a corepack shim and would try the registry unless `COREPACK_HOME` and `COREPACK_ENABLE_NETWORK=0` are set (reported in post_clone).
- Real blocker for A (inferred): a scratch file in `clone-work/` is outside the clone, so module resolution of `zod`, `@types/node`, `vitest/globals` walks up from `clone-work`, not from `clone`. The scratch tsconfig must `extends` the clone tsconfig and add `paths`/`typeRoots` pointing at `../clone/packages/tests/node_modules/*` and `../clone/node_modules/@types` (the `@trpc/*` mapping exists in root `paths`; `baseUrl` resolves at the clone root). The allowance text should say so, or the profile should supply a ready scratch tsconfig template in `clone-work`.
- Extra online provisioning: none for tsc. Extra mount: none beyond node (inferred). `home/` and `corepack/` in the archive serve only the host-side post_clone install.

## 4. Service-only checks

None. The only unresolved item is network: the text of issue #5020 (`blind-1d04b7/8/c11`). Nothing needs a database, prisma, next or a browser.

## 5. Risks for a focused profile

- tsc is safe to enable by shape: `tsc` runs no plugins (inferred). Allow `--noEmit`, `-p <clone/... or ../clone-work/...>`, `--pretty`, `--skipLibCheck`, strictness flags; deny `-b/--build`, `--watch`, `--init`, `--outDir`, `--declarationDir`, `--generateTrace`, `--tsBuildInfoFile`, `--incremental` (writes fail on the read-only clone, noisy only) (inferred).
- A tsconfig `extends` or `paths` can point anywhere on the mounted filesystem; it only reads (inferred).
- Do not enable `pnpm run/exec`, `npx`, `turbo`, `tsx` (executes arbitrary scratch TS) or `vitest`: `vitest --run` writes caches under the read-only clone (`node_modules/.vite`) and the whole `packages/tests` suite is unmeasured and may exceed five minutes (inferred).
- Whole `packages/tests` tsc is cheap (about 5 s reported); generated "big boi" routers come from postinstall codegen, already built.
- Node v24 vs pinned 18: tsc 5.1.3 ran on v24.19 in the reported smoke; no known effect on type results (inferred).
