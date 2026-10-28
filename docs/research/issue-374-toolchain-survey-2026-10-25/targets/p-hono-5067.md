# p-hono-5067 (honojs/hono #5067, body parsing: parseFormData / bufferToFormData)

Labels: **read** = read in a file, **reported** = target.json / receipt text, **inferred** = my reasoning, not run.

## 1. Toolchain (review-head)

| Fact | Value | Label / source |
|---|---|---|
| Package manager | `bun@1.2.20` (`packageManager`); `.tool-versions`: nodejs 24.7.0, bun 1.2.19, deno 2.4.5 | read: `package.json`, `.tool-versions` |
| Engines | `node >=16.9.0` | read: `package.json` |
| Test script | `"test": "tsc --noEmit && vitest --run"`; per-runtime `test:node`, `test:workerd`, `test:fastly`, `test:lambda`, `test:lambda-edge` are all `vitest --run --project <name>`; `test:bun` is `bun test ...`, `test:deno` is `deno test ...` | read: `package.json` |
| Vitest | `vitest ^4.1.7` (4.1.7 installed), `@vitest/coverage-v8`, `jsdom 22.1.0`, `typescript ^5.9.2`, `esbuild ^0.27.1`, `eslint 9.39.3`, `prettier 3.7.4` | read: `package.json`; reported: receipt install tail |
| Vitest config | root `vitest.config.ts`; projects `main` (src, scripts, build tests), `jsx-runtime-default`, `jsx-runtime-dom`, plus `runtime-tests/*/vitest.config.ts`. `coverage.enabled: true` by default | read: `vitest.config.ts` |
| Build/lint | `build` = `bun ./build/build.ts`; `lint` = eslint; `format` = prettier --check | read: `package.json` |
| Installed with | bun 1.3.10 `--frozen-lockfile --ignore-scripts`, 747 packages; node_modules archived (194 MB) | reported: target.json, receipt |
| Binaries in the archive | `.bin/vitest`, `.bin/tsc`, `.bin/eslint`, `.bin/prettier`, `.bin/esbuild`, `@esbuild/linux-x64/bin/esbuild`, `@rolldown/binding-linux-x64-gnu`, `@oxc-parser/binding-linux-x64-gnu`, `@cloudflare/workerd-linux-64`, `bun-types` (types only) | read: `tar -tzf` of the archive |

Does it need bun? **No for the claims graded.** The `main` project is plain vitest on node: `src/utils/body.test.ts`, `buffer.test.ts`, `src/request.test.ts`, `src/validator/validator.test.ts` contain no `bun:`/`Bun.`/`Deno.` references (read: `git grep` at review-head). `bun` is only the installer and script runner (`bun run test`, `bun run build`); the clone's own `./node_modules/.bin/vitest` and `esbuild` replace it. Bun runtime tests (`bun test runtime-tests/bun/*`) cover color/static/jsx, not body parsing (read: file list). `--ignore-scripts` means no postinstall ran; esbuild still works because `@esbuild/linux-x64` is a real package (inferred from the archive listing).

## 2. Commands the graded claims needed (84 refusal rows: 38 first, 46 second)

| Check | argv from clone root | Claims (first/second) | Example id |
|---|---|---|---|
| A. Scratch repro of `await req.formData(); await req.parseBody()` (multipart throws, urlencoded garbage key). Write `.ts` under `clone-work`, bundle, run | `./node_modules/.bin/esbuild <work>/probe.ts --bundle --platform=node --format=esm --outfile=<work>/probe.mjs` then `node <work>/probe.mjs` | 48 rows name esbuild/scratch/probe (25+12+11 by keyword count; 22 more say only "execution unavailable", almost all of them the same repro); this is the GT-p1 repro and is nearly every `problem` row | `blind-23be10/1/c1`, `blind-a56b5f/1/c1` |
| B. Focused vitest on existing tests | `./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/request.test.ts src/validator/validator.test.ts` | about 21 rows name vitest (7 first, 14 second); mostly "do tests cover X / does suite pass" | `blind-0ceae2/8/c10`, `blind-efe757/1/c3`, `blind-34cf12/9/c15` |
| C. Scratch probes of mixed-case / uppercase `BOUNDARY=` / malformed `Content-Type` (`multipart/form-data boundary=x`, `charset` before boundary) on node | same esbuild + node route as A | a handful (about 6) | `blind-d65f34/4/c5`, `blind-441cd6/6/c9`, `blind-37c230/4/c5` |
| D. Other runtimes: Bun's native `Response.formData()` rejects mixed-case media type / uppercase `BOUNDARY`; Workers 128 MB memory limit; peak memory of `arrayBuffer()` vs streaming | needs `bun`, workerd, or a memory-limited process | 5 rows (1 first, 4 second): `blind-82b785/6/c9`, `blind-6814a3/6/c9`, `blind-fedd8f/2/c6`, `blind-236da4/3/c4`, `blind-0ceae2/4/c5` | |
| E. Inspection gaps, not execution: "Base file unavailable", "grep unavailable" | `rg`, `git show` | 2 first rows (`blind-1f7099/5/c7`, `blind-e2f98c/6/c14`) | |

## 3. Can each run offline in the sandbox with CURRENT provisioning?

- A, C (esbuild bundle + node): **yes** (inferred). `esbuild` and its native binary are in `node_modules` after `post_clone`; node is mounted. Needs the clone's `src/` to bundle (read-only is fine); the output goes to `clone-work`. No extra provisioning, no extra mount. Reported by target.json as the intended route. The profile must allow `esbuild` with `--outfile`/`--outdir` confined to `clone-work`/`tmp` and no `--serve`/`--watch`.
- B (vitest): **yes in principle, one thing to prove** (inferred). Pass `--coverage.enabled=false`: the root config enables v8 coverage by default and would write `./coverage` into the read-only clone. Vitest/Vite also write small caches and a temp config bundle under `clone/node_modules/.vite*`; with a read-only clone, run one sandbox smoke run to see whether this fails. If it does, bind a writable dir over `clone/node_modules/.vite` and `.vite-temp` from `clone-cache`, or point cacheDir there. The registered smoke command (`vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/request.test.ts src/validator/validator.test.ts`, "a few seconds") is reported but its receipt has no recorded result and may not have run under bwrap.
- Shim detail (inferred): `node_modules/.bin/vitest` is a bun-made link to a `#!/usr/bin/env node` script; `/usr/bin/env` is mounted and the profile PATH holds the mounted node directory, so it resolves.
- D (other runtimes): **no with current mounts.** Bun claims would need host `~/.bun/bin/bun` (1.3.10) mounted; it can run a scratch `.mjs` offline (`bun <work>/probe.mjs`) and `bun test` offline. Caveats: `bun x`/`bunx`/`bun install`/`bun add` hit the network, so the profile should allow only `bun <script under clone-work>` and `bun test <path>`. Deno claims: same with `~/.deno/bin/deno` (`deno run --no-prompt` and a local `deno.json`; `deno` may try to fetch `npm:`/`jsr:` imports, so only scratch code with no remote imports). Workerd: the binary is in `node_modules` (`@cloudflare/workerd-linux-64`), `vitest --project workerd` launches wrangler dev (inferred from `runtime-tests/workerd`), which is heavy and may try network for telemetry/update checks. Not recommended; claim 236da4/3/c4 (OOM at a 128 MB Workers limit) stays unresolved.
- E: unrelated to execution; `rg`/`git show` are inspection tools.

Whole-suite: `vitest --run --project main` is the whole src suite (thousands of tests; inferred about 1-3 minutes on this machine, unmeasured), so keep selection mandatory (a file or directory under `src/`).

## 4. Service-only checks (stay unresolved)

- Behaviour on real Bun, Deno, Cloudflare Workers/workerd, Lambda: `blind-82b785/6/c9`, `blind-6814a3/6/c9` (uppercase `BOUNDARY=` throws on "some runtime"), `blind-fedd8f/2/c6` (Bun native rejects mixed-case), `blind-236da4/3/c4` (128 MB Worker limit). target.json lists "runtime-tests suites for Deno, Bun, Workers, Lambda and other platforms" as unavailable. If bun is mounted, the Bun ones can become runnable; Workers memory limit cannot.
- No database, Redis, Docker, or browser is involved in this target.

## 5. Risks for a focused profile

- `vitest`: `--coverage.enabled=false` required; deny `--coverage`, `--reporter=junit`/`--outputFile`, `--update`/`-u` (writes snapshots), `--config`, `--root`, `--ui`, `--api`/`--browser` (starts servers or needs browsers), `--watch`, `--inspect`. `--project node|workerd|fastly|lambda` loads plugins/processes that need builds (`bun run build` first) or wrangler; allow `--project main` only.
- `bun run <script>`, `bun install`, `bun x`, `npm`/`npx`: the scripts shell out (`tsc --noEmit && vitest`, `bun ./build/build.ts` etc.) and bun resolves missing packages from the registry. Never allow the package-script path; use the direct `.bin` binaries.
- `tsc --noEmit` at the root: root `tsconfig.json` has `"files": []` and only `references`, so plain `tsc --noEmit` checks nothing (read: `tsconfig.json`); a real typecheck is `tsc -p tsconfig.spec.json --noEmit` (read: `tsconfig.spec.json` sets `noEmit`, `types: vitest/globals`). That could be offered for type claims but none of the 84 rows asked for it; the cost is probably 30-90 s (inferred).
- `node <script>` and a scratch esbuild plugin run arbitrary code by design, but with no network (loopback only) and the clone read-only; scripts may still write to `clone-work`/`tmp`. esbuild's `--outfile` must be confined (it writes wherever told).
- `eslint`/`prettier --check` exist in node_modules; prettier with `--cache` writes to node_modules/.cache (read-only clone). Not asked for by any claim.
