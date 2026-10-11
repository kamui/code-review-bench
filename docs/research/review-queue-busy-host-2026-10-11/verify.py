import argparse
import hashlib
import json
from pathlib import Path
import re


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    root = args.directory
    summary = json.loads((root / "summary.json").read_text())
    rows = [json.loads(line) for line in (root / "workload.jsonl").read_text().splitlines()]
    expected = hashlib.sha256(b"Z" * (32 * 1024 * 1024)).hexdigest()
    assert summary["consecutive_passes"] == 20 and len(summary["runs"]) == 20
    assert summary["workload"]["exit_code"] == 0
    assert summary["workload"]["session"] == summary["workload"]["pid"]
    assert summary["workload"]["queue_marker"] is None
    assert all(row["iteration"] == index and row["exit_code"] == 0 and row["sha256"] == expected
               and not row["stderr"] and row["started_monotonic"] <= row["ended_monotonic"]
               for index, row in enumerate(rows, 1))
    assert all(before["ended_monotonic"] <= after["started_monotonic"] for before, after in zip(rows, rows[1:]))
    inspected = []
    for index, run in enumerate(summary["runs"], 1):
        assert run["run"] == index and run["exit_code"] == 0 and run["workload_alive"]
        assert run["same_head"] and run["clean_tree"]
        output = (root / run["log"]).read_text()
        result = re.findall(r"Ran (\d+) tests in [\d.]+s\n\nOK \(skipped=(\d+)\)", output)
        assert result == [("843", "18")]
        queue = re.findall(r"^test_.+\(test_review_queue\.[^\n]+\) \.\.\. ok$", output, re.MULTILINE)
        assert len(queue) == 58
        during = [row for row in rows if run["started_monotonic"] <= row["ended_monotonic"] <= run["ended_monotonic"]]
        assert during
        assert during[0]["iteration"] >= run["workload_completed_before"]
        assert during[-1]["iteration"] <= run["workload_completed_after"]
        gaps = [after["started_monotonic"] - before["ended_monotonic"] for before, after in zip(during, during[1:])]
        inspected.append({"run": index, "tests": 843, "skips": 18, "queue_tests_passed": len(queue),
                          "workload_children_completed": len(during),
                          "first_workload_completion_after_suite_start_seconds": during[0]["ended_monotonic"] - run["started_monotonic"],
                          "last_workload_completion_before_suite_end_seconds": run["ended_monotonic"] - during[-1]["ended_monotonic"],
                          "maximum_pause_between_workload_children_seconds": max(gaps)})
    files = [{"path": path.name, "bytes": path.stat().st_size,
              "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
             for path in sorted(root.iterdir()) if path.is_file()]
    result = {"head": summary["head"], "verified_suites": inspected, "workload_children": len(rows), "files": files}
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print("verified twenty full suites, their workload overlap, and raw file hashes")
