import argparse
from pathlib import Path
import unittest


HISTORICAL = frozenset({
    "test_last_push_move.py",
    "test_regrade_filing.py",
    "test_v10_filing.py",
    "test_review_code_selection.py",
})


def test_paths(directory: Path, group: str) -> list[Path]:
    return [path for path in sorted(directory.glob("test_*.py"))
            if path.name != "test_grading_client.py"
            and (group == "all" or (path.name in HISTORICAL) == (group == "historical"))]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--group", choices=("all", "core", "historical"), default="all")
    args = parser.parse_args()
    directory = Path(__file__).resolve().parents[1] / "bench/tools"
    loader = unittest.TestLoader()
    suite = unittest.TestSuite(
        loader.discover(str(directory), pattern=path.name)
        for path in test_paths(directory, args.group)
    )
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
