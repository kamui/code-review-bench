#!/usr/bin/env python3
"""Run a tree's `grade.py prepare` with the blind tokens of an earlier preparation.

    python3 pinned_prepare.py GRADE_PY TOKENS.json prepare [grade.py's options]

TOKENS.json is a saved `tokens.json`, {token: attempt id}. `grade.py prepare` names each review with a random
token, taking the attempts in the order of their ids. Here each attempt gets the token it had in the earlier
preparation, so every prepared file has that preparation's bytes and its hash can be compared with the earlier key
and dispatch record. Nothing else of `grade.py` changes."""
import json
import runpy
import secrets
import sys
from pathlib import Path

grade, saved = sys.argv[1], json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
tokens = iter(token.removeprefix("blind-") for token, _attempt in sorted(saved.items(), key=lambda pair: pair[1]))
secrets.token_hex = lambda _bytes=None: next(tokens)
sys.argv = [grade, *sys.argv[3:]]
runpy.run_path(grade, run_name="__main__")
