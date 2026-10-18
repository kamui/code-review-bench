#!/usr/bin/env python3
"""Check that the change which took the review-code reviews of the re-cut tasks into the published selection writes
the committed files, as one generation: all of them, or none.

Usage: python3 bench/tools/test_review_code_selection.py
Inputs: a copy of this repository's bench and docs trees in a temporary directory, as test_last_push_move.py makes
it. The copy is put back to the selection the re-cut record staged and to the claims without the links this change
added, and the change's own script runs in it. Needs the freeze commits of the selected runs in the clone. No
network, and nothing is written into this checkout.
Exit codes: 0 all checks pass; 1 a test fails.
"""

from __future__ import annotations

from contextlib import redirect_stdout
import hashlib
import importlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import claims as claim_tools
import current_grading  # noqa: F401
from test_last_push_move import (CURRENT, INVENTORY, RECUT, REGISTRY, ROOT, SELECTION, V10, Failing, before_the_selection,
                                 copy_of_the_repository, differing, read, regrade_filing, undo_the_activation_filing, tree)

WRITTEN = [REGISTRY, INVENTORY, CURRENT / "claims.json"]
LEFT = "bench/runs/2026-09-24-builtin-baseline/attempts/att-083/normalized.json"
PROMPT = Path("docs/research/cohort-rebuild-2026-10-05/link-intake/prompt.md")


def intake_script(name: str):
    """One of the record's intake scripts in this checkout: ``packets``, ``collect`` or ``usage``."""
    spec = importlib.util.spec_from_file_location(f"selection_{name}", ROOT / SELECTION / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def selection_script(root: Path):
    """The change's script in the repository at ``root``, loaded so that it and the scripts it imports take that root."""
    with patch.object(sys, "path", [str(root / SELECTION), *sys.path]), patch.dict(sys.modules):
        for loaded in ("record", "selection", "stage_edition"):
            sys.modules.pop(loaded, None)
        return importlib.import_module("selection")


class OneGenerationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name).resolve()
        copy_of_the_repository(cls.root)
        cls.selection = selection_script(cls.root)
        before_the_selection(cls.root, cls.selection.stage)
        cls.before = tree(cls.root)

    def setUp(self) -> None:
        for name in tree(self.root).keys() - self.before.keys():
            (self.root / name).unlink()
        for name, data in self.before.items():
            path = self.root / name
            if not path.exists() or path.read_bytes() != data:
                path.write_bytes(data)

    def run_change(self, at=None) -> list:
        """Run the copy's change and return the files it wrote, in order. ``at`` fails that write."""
        writes = Failing(self.selection.record.replace, at)
        with patch.object(self.selection.record, "replace", writes):
            self.selection.Selection().run()
        return [path.relative_to(self.root).as_posix() for path in writes.paths]

    def edit(self, name: str, change) -> None:
        path = self.root / CURRENT / f"{name}.json"
        document = read(path)
        change(document)
        path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")

    def test_the_copy_starts_from_the_selection_the_recut_record_staged(self) -> None:
        staged = read(ROOT / RECUT / "staged-edition.v2.json")["staged"]
        self.assertEqual({"registry_sha256": hashlib.sha256(self.before[REGISTRY.as_posix()]).hexdigest(),
                          "inventory_sha256": hashlib.sha256(self.before[INVENTORY.as_posix()]).hexdigest()},
                         {name: staged[name] for name in ("registry_sha256", "inventory_sha256")})

    def test_the_change_rebuilds_the_committed_files_from_the_staged_selection(self) -> None:
        written = self.run_change()
        self.assertEqual(sorted(written), sorted(path.as_posix() for path in WRITTEN))
        after = tree(self.root)
        self.assertEqual(differing(after, self.before), sorted(written))
        self.assertEqual([path.as_posix() for path in WRITTEN if after[path.as_posix()] != (ROOT / path).read_bytes()], [])
        self.assertEqual(self.run_change(), [])
        self.assertEqual(differing(tree(self.root), after), [])

    def test_a_failure_before_the_first_write_or_between_two_writes_leaves_every_file_as_it_was(self) -> None:
        written = self.run_change()
        self.assertEqual(len(written), len(WRITTEN))
        for at, name in enumerate(written):
            self.setUp()
            with self.subTest(fails_writing=name, after_writing=written[:at]):
                with self.assertRaisesRegex(OSError, "injected failure"):
                    self.run_change(at)
                self.assertEqual(differing(tree(self.root), self.before), [])

    def test_records_that_do_not_validate_leave_every_file_as_it_was(self) -> None:
        with patch.object(self.selection.current, "validate_documents", side_effect=self.selection.current.Inconsistent("injected")):
            with self.assertRaisesRegex(SystemExit, "do not validate"):
                self.run_change()
        self.assertEqual(differing(tree(self.root), self.before), [])

    def test_a_claim_whose_comments_all_left_the_selection_stops_the_change(self) -> None:
        def strand(claims: dict) -> None:
            claim = next(row for row in claims["claims"] if row["id"] == "CL-j-void-assertion")
            claim["links"] = [link for link in claim["links"] if link["review"]["path"] == LEFT]
            self.assertTrue(claim["links"])
        self.edit("claims", strand)
        edited = tree(self.root)
        with self.assertRaisesRegex(SystemExit, "only the owner retires one: CL-j-void-assertion$"):
            self.run_change()
        self.assertEqual(differing(tree(self.root), edited), [])

    def test_a_ruling_on_a_comment_that_left_the_selection_stops_the_change(self) -> None:
        def move(credits: dict) -> None:
            credits["rulings"][0]["review"]["path"] = LEFT
        self.edit("credits", move)
        edited, ruling = tree(self.root), read(self.root / CURRENT / "credits.json")["rulings"][0]["id"]
        with self.assertRaisesRegex(SystemExit, f"only the owner retires one: {ruling}$"):
            self.run_change()
        self.assertEqual(differing(tree(self.root), edited), [])

    def test_a_staged_registry_other_than_the_pinned_one_stops_the_change_before_any_write(self) -> None:
        registry, decided, deferred = self.selection.stage.staged_registry()
        kept = [source for source in registry["sources"]
                if (source["configuration"], source["tasks"]) != ("codex-ce-luna-high", ["j-trpc-5017"])]
        self.assertEqual(len(kept), len(registry["sources"]) - 1)
        with patch.object(self.selection.stage, "staged_registry", return_value=({**registry, "sources": kept}, decided, deferred)):
            with self.assertRaisesRegex(SystemExit, "no longer stages the registry staged-edition.v2.json pins"):
                self.run_change()
        self.assertEqual(differing(tree(self.root), self.before), [])

    def test_text_for_a_configuration_that_left_the_selection_is_refused(self) -> None:
        change = self.selection.Selection()
        change.plan["configuration_text"] = {"codex-builtin": {"note": "Left the selection."}}
        with self.assertRaisesRegex(SystemExit, "not in the selection"):
            change.registry()

    def test_a_comment_linked_to_two_claims_is_linked_to_neither_as_equivalent(self) -> None:
        change, selected = self.selection.Selection(), read(ROOT / INVENTORY)
        proposed = next(link for link in change.links if link["claim"] == "CL-i-tls-version-ignored")
        change.links = [{**proposed, "relation": "equivalent"}, {**proposed, "claim": "CL-i-adapter-ca-shared"}]
        fresh = change.proposed(selected)
        change.narrow(fresh)
        self.assertEqual([link["relation"] for link in fresh.values()], ["related", "related"])
        self.assertEqual([link["reason"].endswith(f"the item is also linked to {other}.") for link, other in
                          zip(fresh.values(), ("CL-i-adapter-ca-shared", "CL-i-tls-version-ignored"))], [True, False])

    def test_a_link_to_a_comment_of_a_review_that_did_not_enter_is_refused(self) -> None:
        change = self.selection.Selection()
        change.links = [{**change.links[0], "review": {**change.links[0]["review"], "path": LEFT}}]
        with self.assertRaisesRegex(SystemExit, "not a comment of an admitted review that entered"):
            change.proposed(read(ROOT / INVENTORY))


class RecordTests(unittest.TestCase):
    def test_the_committed_record_is_the_one_the_script_derives(self) -> None:
        """The record counts the claims as the selection left them, so the rulings filed later are undone first."""
        root = Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        copy_of_the_repository(root)
        undo_the_activation_filing(root)
        regrade_filing(root).Filing().undo()
        selection = selection_script(root)
        self.assertEqual(selection.RECORD.read_text(encoding="utf-8"), selection.Selection().recorded())


class IntakeTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.intake = read(ROOT / SELECTION / "intake.v1.json")

    def run_script(self, name: str, *arguments, HERE: Path | None = None) -> None:
        """Run one of the record's scripts. ``HERE`` is where it writes its result in place of the record."""
        script = intake_script(name)
        with patch.object(script, "HERE", HERE or script.HERE), patch.object(sys, "argv", [name, *map(str, arguments)]), \
                redirect_stdout(io.StringIO()):
            script.main()

    def at_the_intake(self, by_task: dict) -> dict:
        """``by_task`` without the tasks whose first claim was filed after the intake. Such a claim is of a kind that did
        not exist then, a claim of a pre-existing problem, or one the v10 regrade rulings filed."""
        claims = read(ROOT / CURRENT / "claims.json")["claims"]
        filed_later = set(read(ROOT / V10 / "before.v1.json")["records"]["claims"]["added"])
        later = {task for task in by_task if task not in self.intake["reviews"]}
        self.assertTrue(all(claim.get("pre_existing_problem_id") or claim["id"] in filed_later
                            for claim in claims if claim["target"] in later), later)
        return {task: value for task, value in by_task.items() if task not in later}

    def packets(self) -> tuple:
        self.run_script("packets", self.temp / "packets", self.temp / "key.json")
        return self.temp / "packets", read(self.temp / "key.json")

    def test_the_packets_hold_every_comment_of_the_reviews_that_entered_and_name_none(self) -> None:
        packets, key = self.packets()
        claims = read(ROOT / CURRENT / "claims.json")["claims"]
        entered = {row["target"] for row in read(ROOT / "bench" / read(ROOT / SELECTION / "plan.v1.json")["enters"]["run"] / "manifest.json")["cohort"]}
        self.assertEqual(sorted(key), sorted({claim["target"] for claim in claims} & entered))
        self.assertEqual(self.at_the_intake({task: sorted({source["review"]["path"] for source in names["items"].values()})
                                             for task, names in key.items()}), self.intake["reviews"])
        for task, names in key.items():
            part = read(packets / task / "part-1.json")
            self.assertEqual(sorted(path.name for path in (packets / task).iterdir()), ["part-1.json", "prompt.md"])
            self.assertEqual((packets / task / "prompt.md").read_bytes(), (ROOT / PROMPT).read_bytes())
            self.assertEqual(sorted(names["claims"].values()), sorted(claim["id"] for claim in claims if claim["target"] == task))
            comments = len(names["items"]) if task not in self.intake["reviews"] else sum(
                len(read(ROOT / path)["items"]) for path in self.intake["reviews"][task])
            self.assertEqual((len(part["items"]), len(names["items"])), (comments, comments))
            text = (packets / task / "part-1.json").read_text(encoding="utf-8")
            self.assertEqual(claim_tools.exposed(text, claim_tools.identities([], ROOT)), [], task)

    def test_the_collector_maps_labels_to_relations_and_refuses_an_item_left_out(self) -> None:
        packets, key = self.packets()
        task = "m-grpc-go-7390"
        first, *others = sorted(key[task]["items"])
        for other in set(key) - {task}:
            (packets / other / "matches-1.json").write_text(json.dumps({"matched": [], "unmatched": sorted(key[other]["items"])}))
        matches = {"matched": [{"item": first, "claim": "C1", "label": "only-claim", "reason": "one"},
                               {"item": others[0], "claim": "C1", "label": "with-other", "reason": "two"}], "unmatched": others[1:]}
        (packets / task / "matches-1.json").write_text(json.dumps(matches))
        self.run_script("collect", packets, self.temp / "key.json", HERE=self.temp)
        collected = read(self.temp / "intake.v1.json")
        self.assertEqual([(link["claim"], link["relation"], link["item_id"], link["review"]) for link in collected["links"]],
                         [("CL-m-teardown-lock-wait", relation, key[task]["items"][item]["item_id"], key[task]["items"][item]["review"])
                          for item, relation in ((first, "equivalent"), (others[0], "related"))])
        self.assertEqual(self.at_the_intake(collected["reviews"]), self.intake["reviews"])
        (packets / task / "matches-1.json").write_text(json.dumps({**matches, "unmatched": others[2:]}))
        with self.assertRaisesRegex(SystemExit, "does not cover every item once"):
            self.run_script("collect", packets, self.temp / "key.json", HERE=self.temp)

    def test_the_collector_refuses_parts_that_do_not_hold_every_keyed_item_exactly_once(self) -> None:
        source = {"review": {"path": "bench/runs/run/attempts/att-001/normalized.json", "sha256": "0" * 64},
                  "attempt_id": "att-001", "item_id": "item-0"}
        key = self.temp / "key.json"
        key.write_text(json.dumps({"t-task": {"claims": {"C1": "CL-t-claim"}, "items": {"I0000": source}}}), encoding="utf-8")
        cases = {"no part file": {}, "an item the key does not hold": {1: ["I0000", "I9999"]},
                 "an item in two parts": {1: ["I0000"], 2: ["I0000"]}, "a keyed item left out": {1: []}}
        for case, parts in cases.items():
            with self.subTest(case=case):
                packets = self.temp / case / "t-task"
                packets.mkdir(parents=True)
                for number, items in parts.items():
                    (packets / f"part-{number}.json").write_text(json.dumps({"items": dict.fromkeys(items, {})}), encoding="utf-8")
                    (packets / f"matches-{number}.json").write_text(json.dumps({"matched": [], "unmatched": items}), encoding="utf-8")
                with self.assertRaisesRegex(SystemExit, "do not hold every item of the key exactly once"):
                    self.run_script("collect", packets.parent, key, HERE=self.temp)
                self.assertFalse((self.temp / "intake.v1.json").exists())

    def test_usage_prices_each_session_from_its_last_usage_event(self) -> None:
        logs = self.temp / "logs"
        logs.mkdir()
        events = [{"type": "thread.started", "thread_id": "thread-1"},
                  {"type": "turn.completed", "usage": {"input_tokens": 1_500_000, "cached_input_tokens": 1_000_000, "output_tokens": 100_000}}]
        (logs / "one.run.jsonl").write_text("".join(json.dumps(event) + "\n" for event in events), encoding="utf-8")
        self.run_script("usage", logs, "codex-cli test", HERE=self.temp)
        usage = read(self.temp / "usage.v1.json")
        rates = usage["rates_usd_per_million"]
        self.assertEqual(usage["sessions"][0]["list_price_usd"],
                         round(0.5 * rates["input"] + 1.0 * rates["cache_read"] + 0.1 * rates["output"], 4))
        self.assertEqual((usage["sessions"][0]["thread_id"], usage["total"]["uncached_input_tokens"]), ("thread-1", 500_000))


if __name__ == "__main__":
    unittest.main(verbosity=2)
