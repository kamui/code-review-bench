import argparse
from pathlib import Path
import unittest


def test_paths(directory: Path) -> list[Path]:
    return [path for path in sorted(directory.glob("test_*.py")) if path.name != "test_grading_client.py"]


def main() -> int:
    argparse.ArgumentParser().parse_args()
    directory = Path(__file__).resolve().parents[1] / "bench/tools"
    loader = unittest.TestLoader()
    suite = unittest.TestSuite(loader.discover(str(directory), pattern=path.name) for path in test_paths(directory))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
