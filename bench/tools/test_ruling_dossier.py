"""Prove a candidate dossier is refused unless it shows its promise and the searches behind it."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

import ruling_dossier


def checked(found=None):
    return {"state": "checked", "query": "rg -l cancel docs/", "saved": "upstream/hits.txt", "hits": 2, "read": 2, "found": found}


PROMISE = {"made_by": "written", "whose_interface": "urllib3, public function", "classification": "public",
           "source": "module documentation: 'before you begin making HTTP requests'",
           "searched": {**{place: checked() for place in ruling_dossier.PLACES},
                        "owner-docs": checked("before you begin making HTTP requests")}}
ENTRY = {"group": "N1", "kind": "candidate", "recommendation": "problem", "delivered": "no", "promise": PROMISE}


class RulingDossierTest(unittest.TestCase):
    def faults(self, entry, dossier="## Promised?\n\n## Delivered?\n", refresh=None):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        (root / "dossiers").mkdir()
        (root / "upstream").mkdir()
        (root / "upstream/hits.txt").write_text("two hits\n", encoding="utf-8")
        (root / "summary.json").write_text(json.dumps([entry]), encoding="utf-8")
        (root / "dossiers/N1.md").write_text(dossier, encoding="utf-8")
        if refresh:
            (root / "refresh.json").write_text(json.dumps([refresh]), encoding="utf-8")
            (root / "dossiers/N1-supplement.md").write_text("## Promised?\n\n## Delivered?\n", encoding="utf-8")
        return ruling_dossier.faults(root)

    def changed(self, **promise):
        entry = copy.deepcopy(ENTRY)
        entry["promise"].update(promise)
        return entry

    def test_a_shown_promise_with_its_searches_passes(self):
        self.assertEqual(self.faults(ENTRY), [])

    def test_a_search_is_a_saved_response_with_its_query(self):
        searched = copy.deepcopy(PROMISE["searched"])
        searched["maintainers"] = {"state": "checked", "saved": "upstream/missing.json", "hits": 3, "read": 1}
        found = "\n".join(self.faults(self.changed(searched=searched)))
        for fault in ("searched.maintainers needs the query as it was run", "searched.maintainers.saved must name a saved, non-empty response",
                      "searched.maintainers read 1 of 3 hits", "searched.maintainers needs `found`"):
            self.assertIn(fault, found)

    def test_a_place_is_checked_or_says_why_not(self):
        searched = {**copy.deepcopy(PROMISE["searched"]), "public-code": None, "documented-way": {"state": "not-applicable"}}
        found = self.faults(self.changed(searched=searched))
        self.assertIn("N1: searched.public-code needs `state`: checked, not-applicable or blocked", found)
        self.assertIn("N1: searched.documented-way is not-applicable and needs the reason", found)

    def test_a_blocked_search_cannot_support_no_promise(self):
        searched = {**copy.deepcopy(PROMISE["searched"]), "public-code": {"state": "blocked", "reason": "code search unavailable"}}
        entry = {**self.changed(made_by="none", source=None, searched=searched), "delivered": None, "recommendation": "suggestion"}
        self.assertEqual(self.faults(entry, "## Promised?\n"), ["N1: a blocked search cannot support `none`; finish it or recommend unproven"])

    def test_relied_on_needs_the_programs(self):
        searched = copy.deepcopy(PROMISE["searched"])
        entry = {**self.changed(made_by="none", source=None, searched=searched), "delivered": None, "recommendation": "relied-on"}
        self.assertEqual(self.faults(entry, "## Promised?\n"), ["N1: `relied-on` needs the programs found in searched.public-code"])
        searched["public-code"]["found"] = "32 files assign it, the Datadog agent since 2021"
        self.assertEqual(self.faults(entry, "## Promised?\n"), [])
        self.assertIn("promise.made_by must be one of", self.faults(self.changed(made_by="practice"))[0])

    def test_the_recommendation_follows_from_promise_and_delivery(self):
        searched = copy.deepcopy(PROMISE["searched"])
        none = {**self.changed(made_by="none", source=None, searched=searched), "delivered": None}
        self.assertIn("is one of suggestion, relied-on", self.faults(none, "## Promised?\n")[0])
        self.assertEqual(self.faults({**ENTRY, "recommendation": "duplicate"}), [])
        self.assertIn("is one of minor-defect, duplicate", self.faults({**ENTRY, "delivered": "yes"})[0])

    def test_a_refuted_claim_needs_no_promise_and_a_recovery_question_is_not_checked(self):
        self.assertEqual(self.faults({"group": "N1", "kind": "candidate", "recommendation": "refuted"}, ""), [])
        self.assertEqual(self.faults({"group": "N1", "kind": "recovery"}, ""), [])

    def test_a_refresh_readies_an_old_dossier_without_rewriting_it(self):
        old = {"group": "N1", "kind": "candidate", "recommendation": "advisory"}
        self.assertIn("N1: needs `promise`", self.faults(old, "## Problem\n")[0])
        refresh = {key: ENTRY[key] for key in ("group", "recommendation", "delivered", "promise")}
        self.assertEqual(self.faults(old, "## Problem\n", refresh), [])


if __name__ == "__main__":
    unittest.main()
