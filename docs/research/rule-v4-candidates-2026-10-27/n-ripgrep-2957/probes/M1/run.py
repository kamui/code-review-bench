import runpy
from pathlib import Path
root = Path(__file__).resolve().parent.parent
runpy.run_path(str(root / 'test_operation.py'), run_name='__main__')
