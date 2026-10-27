# Reproducing the preparation

The Python probes use only the standard library and zsh. Clones, binaries, startup homes and patched script variants were created in the scratch directory named by the brief. The completed clone and dependency/build directories are removed after verification.

To rebuild, clone the packet's mirror into the scratch directory's `repo`, check out each pinned commit in turn and run `cargo build --locked --offline` with a scratch `CARGO_HOME` referencing the existing registry cache and a scratch `CARGO_TARGET_DIR`. Preserve each binary as `rg-base` or `rg-head` in scratch and each generated script under `probes/`, as in the environment receipts. These builds generated exactly the saved scripts.

`test_operation.py` runs the full base/head check and extraction helper. `alias_status.py` isolates parsing and return status. `interactive.py` uses a pseudoterminal and records the actual editor buffer for the base/head completion matrix. `extra_checks.py` adds explicit-registration and documented-fpath controls. `grouping.py` preserves the initial narrow patches; `grouping_full.py` tests the current widened GT-n5 against a completion-context patch. Run the full variant generator before `known_variants.py`, which covers wrappers as well as bare-name sourcing. `platform.py` and saved comparable-prefix probes distinguish platform behavior from current-tool comparisons.

The raw results and patches are already saved. A rerun should copy the evidence tree to scratch first to avoid replacing these preparation records. The initial comparable capture returned no saved buffer before the external command completed; the retry waited for capture. Both records are retained, and only the successful retry is used.
