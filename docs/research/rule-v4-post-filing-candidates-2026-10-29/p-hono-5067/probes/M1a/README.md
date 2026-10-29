# M1a

Rows of the full run in `../M1/` for this sub-group's headers, the known problem GT-p6's header and the controls: `comma-with-boundary`, `comma-no-boundary`, `comma-joined-twice`, `urlencoded-comma`, `GT-p6-missing-semicolon`, `control-valid`, `control-mixed-case`, `control-space-before-semicolon`, `control-urlencoded`.

Reproduce with `python3 -I ../M1/run.py`, `python3 -I ../M1/http.py` and `python3 -I ../M1/split.py`, after recreating the clone (see `../M1/README.md`). `result-<label>.txt` is one JSON line per runtime and header; `patches/` holds every patch named by a label; `known-<label>.txt` is the known-problem probe (`../M1/known-probe.mjs`) under that label; `tests-<label>.txt` is the project's own body, buffer and validator tests under that label; `http-*.txt` is the over-HTTP run.
