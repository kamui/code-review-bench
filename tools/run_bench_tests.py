from pathlib import Path
import unittest


if __name__ == "__main__":
    directory = Path(__file__).resolve().parents[1] / "bench/tools"
    loader = unittest.TestLoader()
    suite = unittest.TestSuite(
        loader.discover(str(directory), pattern=path.name)
        for path in sorted(directory.glob("test_*.py"))
        if path.name != "test_grading_client.py"
    )
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(0 if result.wasSuccessful() else 1)
