"""Prove a candidate dossier is refused unless it says where a promise comes from or what was searched."""
import json
from pathlib import Path
import tempfile
import unittest

import ruling_dossier

PROMISE = {"made_by": "written", "whose_interface": "urllib3, public function", "source": "docs: 'before you begin making HTTP requests'",
           "owner_statement": None, "practice": None, "searched": ["project documentation", "dependency documentation"]}


class RulingDossierTest(unittest.TestCase):
    def directory(self, entry, dossier):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        (root / "dossiers").mkdir()
        (root / "summary.json").write_text(json.dumps([{"group": "N1", "kind": "candidate", **entry}]), encoding="utf-8")
        (root / "dossiers/N1.md").write_text(dossier, encoding="utf-8")
        return root

    def test_stated_promise_and_delivery_pass(self):
        root = self.directory({"promise": PROMISE, "delivered": "no"}, "## Promised?\n\n## Delivered?\n")
        self.assertEqual(ruling_dossier.faults(root), [])

    def test_missing_promise_is_refused(self):
        root = self.directory({"recommendation": "advisory"}, "## Problem\n")
        self.assertIn("N1: summary.json needs `promise`", ruling_dossier.faults(root)[0])

    def test_no_promise_still_needs_the_search_and_the_section(self):
        none = {**PROMISE, "made_by": "none", "source": None, "searched": []}
        found = ruling_dossier.faults(self.directory({"promise": none, "delivered": None}, "## Problem\n"))
        self.assertEqual(len(found), 2)
        self.assertIn("promise.searched", found[0])
        self.assertIn("`## Promised?`", found[1])

    def test_recovery_question_is_not_checked(self):
        root = self.directory({"kind": "recovery"}, "")
        self.assertEqual(ruling_dossier.faults(root), [])


if __name__ == "__main__":
    unittest.main()
