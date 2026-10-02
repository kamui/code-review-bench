# Rate refresh and dispatch checks

Check the current catalog against official provider prices:

```sh
bun run rates:check
```

Append dated entries for prices that changed:

```sh
bun run rates:refresh
```

Both commands default to `bench/rates.current.json`. They fetch the [OpenAI standard pricing table](https://developers.openai.com/api/docs/pricing) and [Anthropic model pricing table](https://platform.claude.com/docs/en/about-claude/pricing). They make no model requests and need no credentials. Prices cover standard short-context usage, matching the catalog's existing policy. Batch, flex, priority, long-context premiums, and partner-cloud prices are outside this command's scope.

Use `--model` to select models and `--json` to save the check receipt, including the time, source URLs, hashes, observed prices, and differences:

```sh
python3 bench/tools/rates.py check --model gpt-6.1-sol --json
python3 bench/tools/rates.py refresh --model claude-sonnet-5-5
```

The command checks only models already in the catalog. Add a model's initial entry and billing policy before checking it. Provider table changes, unsupported prices, and missing models return exit 1. Source or file failures return exit 2. A successful check returns 0; differing prices return 1. A successful refresh returns 0.

Refresh preserves old entries and adds one entry with today's UTC date for each changed model. An unchanged catalog keeps its original bytes. Concurrent refreshes use a file lock, and replacement is atomic after all selected models have been checked. A second change on the same date is refused because the catalog identifies entries by model and date. The command refuses refresh destinations in frozen runs, preserved artifacts, and the imported `bench/rates.json`.

Run `rates:refresh` before freezing a new benchmark and pin the selected dated entries in its manifest. Refreshing the catalog does not change saved costs or the website. `bun run data` exports existing benchmark evidence.

## Runner change v1

The `rates-check-v1` policy adds a provider check to current `bench/tools/run_cell.py` before claiming a real attempt. It checks the selected arm's explicit model against the manifest's dated rate pin, using `BENCH_RATES` when set or `bench/rates.current.json` otherwise. Changed prices, a missing pin, or an unavailable source refuse dispatch before provisioning or payment. Refresh the catalog and freeze a new run when prices change.

Successful claims save a `rates_check` receipt in `cell.json` and the verified dated entry in the attempt's `rates.json`. Skill runners and `file_attempt.py` receive this snapshot for metering. A later catalog refresh therefore cannot change the prices used by an in-flight attempt. Status, dry runs, and filing previously dispatched attempts do not contact providers.

Frozen runner copies retain their existing behavior. Record adopting this runner change as a versioned deviation when continuing an earlier run; preserve its existing manifest, evidence, and failed attempts. No existing run is rewritten by these commands. External controllers and direct adapter invocations do not inherit the automatic check; invoke `rates.py check` with their selected catalog and model before dispatch, and keep their rates pinned.

Verification:

```sh
python3 -m unittest discover -s bench/tools -p test_rates.py -v
python3 bench/tools/run_cell.py --self-test
bun run verify:import
```
