from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest
import sys

sys.path.insert(0, str(Path(__file__).parent))
from codex_skill_runner import RunnerError, hidden_aliases, load_rollouts, sandbox_command


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


class SandboxAliasTests(unittest.TestCase):
    MOUNTS = ("29 1 8:48 / / rw - ext4 /dev/sdd rw\n"
              "40 29 0:38 / /mnt/wslg ro - tmpfs none rw\n"
              "41 40 8:48 / /mnt/wslg/distro ro - ext4 /dev/sdd rw\n"
              "42 29 8:48 /home/u/other /mnt/elsewhere rw - ext4 /dev/sdd rw\n"
              "43 29 8:16 / /mnt/c rw - 9p drvfs rw\n")

    def test_attempt_is_bound_where_a_hidden_mount_of_its_filesystem_shows_it(self):
        attempt = Path("/home/u/runs/att-001")
        aliases = hidden_aliases(attempt, ["/home", "/mnt"], self.MOUNTS)
        self.assertEqual(aliases, ["/mnt/wslg/distro/home/u/runs/att-001"])
        prefix, receipt = sandbox_command("bwrap-v1", attempt, attempt / "clone", [], aliases)
        bind = prefix.index(aliases[0])
        self.assertEqual(prefix[bind - 2:bind + 1], ["--bind", str(attempt), aliases[0]])
        self.assertLess(prefix.index("/mnt"), bind)
        self.assertEqual(receipt["readwrite"], [str(attempt), *aliases])

    def test_attempt_on_a_separately_mounted_home_is_found_by_its_path_in_the_filesystem(self):
        mounts = ("29 1 8:1 / / rw - ext4 /dev/sda rw\n30 29 8:2 / /home rw - ext4 /dev/sdb rw\n"
                  "31 29 8:2 /u /srv/u rw - ext4 /dev/sdb rw\n")
        self.assertEqual(hidden_aliases(Path("/home/u/runs/att-001"), ["/home", "/srv"], mounts),
                         ["/srv/u/runs/att-001"])

    def test_no_alias_without_another_mount(self):
        self.assertEqual(hidden_aliases(Path("/home/u/runs/att-001"), ["/home", "/mnt"],
                                        "29 1 8:48 / / rw - ext4 /dev/sdd rw\n"), [])


if __name__ == "__main__":
    unittest.main()
