import runpy
import sys
from pathlib import Path

sys.argv.append("--keylog-only")
runpy.run_path(str(Path(__file__).resolve().parents[1] / "N2a/probe.py"), run_name="__main__")
