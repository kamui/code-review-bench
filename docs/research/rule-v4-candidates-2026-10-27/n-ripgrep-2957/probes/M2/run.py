import runpy
from pathlib import Path
root = Path(__file__).resolve().parent.parent
runpy.run_path(str(root / 'alias_status.py'), run_name='__main__')
runpy.run_path(str(root / 'interactive.py'), run_name='__main__')
