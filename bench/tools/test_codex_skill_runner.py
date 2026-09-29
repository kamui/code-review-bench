from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest
import sys

sys.path.insert(0, str(Path(__file__).parent))
from codex_skill_runner import RunnerError, load_rollouts


def rollout(path: Path, session_id: str, *, parent: str | None = None, spawn: dict | None = None):
    records = [{"type": "session_meta", "payload": {"id": session_id, "parent_thread_id": parent}},
               {"type": "turn_context", "payload": {"model": "gpt-6-luna", "effort": "high"}}]
    if spawn:
        records.append({"type": "function_call", "name": "spawn_agent", "arguments": json.dumps(spawn)})
    path.write_text("\n".join(json.dumps(record) for record in records) + "\n", encoding="utf-8")


class RolloutLineageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def test_primary_session_is_accepted(self):
        rollout(self.root / "rollout-root.jsonl", "root")
        root_id, contexts, lineage, spawn_calls, paths = load_rollouts(self.root, "gpt-6-luna", "high")
        self.assertEqual(root_id, "root")
        self.assertEqual(len(contexts), 1)
        self.assertEqual(lineage[0]["parent_thread_id"], None)
        self.assertEqual(spawn_calls, [])
        self.assertEqual(len(paths), 1)

    def test_child_must_link_to_root_and_use_fresh_fork(self):
        rollout(self.root / "rollout-root.jsonl", "root", spawn={"fork_turns": "none"})
        rollout(self.root / "rollout-child.jsonl", "child", parent="root")
        result = load_rollouts(self.root, "gpt-6-luna", "high")
        child = next(item for item in result[2] if item["session_id"] == "child")
        self.assertEqual(child["parent_thread_id"], "root")
        self.assertEqual(result[3][0]["fork_turns"], "none")

    def test_child_without_logged_fresh_fork_is_rejected(self):
        rollout(self.root / "rollout-root.jsonl", "root", spawn={"fork_turns": "all"})
        rollout(self.root / "rollout-child.jsonl", "child", parent="root")
        with self.assertRaisesRegex(RunnerError, "fork_turns=none"):
            load_rollouts(self.root, "gpt-6-luna", "high")

    def test_session_without_explicit_model_context_is_rejected(self):
        p = self.root / "rollout-root.jsonl"
        p.write_text(json.dumps({"type": "session_meta", "payload": {"id": "root"}}) + "\n", encoding="utf-8")
        with self.assertRaisesRegex(RunnerError, "turn context"):
            load_rollouts(self.root, "gpt-6-luna", "high")


if __name__ == "__main__":
    unittest.main()
