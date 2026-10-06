"""Prove a candidate dossier is refused unless it shows its promise and the searches made to find one."""
import json
from pathlib import Path
import tempfile
import unittest

import ruling_dossier


def search(**changes):
    return {"query": "rg -l cancel docs/", "saved": "upstream/search.txt", "hits": 3, "read": 3, "found": None, **changes}


def promise(**changes):
    searched = {place: search() for place in ruling_dossier.PLACES}
    searched["owner-docs"] = {"none": "the project owns the interface"}
    return {"made_by": "written", "whose_interface": "Base UI, Field", "classification": "public",
            "source": "handbook: '`cancel` stops the component from changing its internal state'", "searched": searched, **changes}


class RulingDossierTest(unittest.TestCase):
    def directory(self, entry, dossier="## Promised?\n\n## Delivered?\n"):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        root = Path(temp.name)
        (root / "dossiers").mkdir()
        (root / "upstream").mkdir()
        (root / "upstream/search.txt").write_text("docs/handbook/customization.mdx\n", encoding="utf-8")
        (root / "summary.json").write_text(json.dumps([{"group": "N1", "kind": "candidate", **entry}]), encoding="utf-8")
        (root / "dossiers/N1.md").write_text(dossier, encoding="utf-8")
        return root

    def faults(self, entry, dossier="## Promised?\n\n## Delivered?\n"):
        return ruling_dossier.faults(self.directory(entry, dossier))

    def test_promise_with_saved_searches_passes(self):
        self.assertEqual(self.faults({"promise": promise(), "delivered": "no", "recommendation": "problem"}), [])

    def test_missing_promise_is_refused(self):
        self.assertIn("N1: summary.json needs `promise`", self.faults({"recommendation": "suggestion"})[0])

    def test_a_place_not_searched_is_refused(self):
        # Second-pass ruling 2 and S9: nobody had searched what maintainers said about the name.
        stated = promise()
        del stated["searched"]["maintainers"]
        found = self.faults({"promise": stated, "delivered": "no", "recommendation": "problem"})
        self.assertEqual(len(found), 1)
        self.assertIn("needs `maintainers`", found[0])

    def test_a_search_without_its_saved_response_is_refused(self):
        stated = promise()
        stated["searched"]["public-code"] = search(saved="upstream/never-run.json")
        self.assertIn("searched.public-code.saved", self.faults({"promise": stated, "delivered": "no", "recommendation": "problem"})[0])

    def test_unread_hits_are_refused_unless_explained(self):
        # Review 5 of the seven: the component page was read and the handbook, another hit for the same name, was not.
        stated = promise()
        stated["searched"]["project-docs"] = search(hits=3, read=1)
        self.assertIn("read 1 of 3 hits", self.faults({"promise": stated, "delivered": "no", "recommendation": "problem"})[0])
        stated["searched"]["project-docs"]["unread"] = "two hits are changelog entries"
        self.assertEqual(self.faults({"promise": stated, "delivered": "no", "recommendation": "problem"}), [])

    def test_recommendation_must_follow_from_promise_and_delivery(self):
        # Second-pass ruling 2: "eligible" recommended on breakage alone, with no promise found.
        none = promise(made_by="none", source=None)
        found = self.faults({"promise": none, "delivered": None, "recommendation": "problem"}, "## Promised?\n")
        self.assertEqual(len(found), 1)
        self.assertIn("must be `suggestion`", found[0])

    def test_practice_needs_the_programs(self):
        practice = promise(made_by="practice", source="two public programs assign it")
        self.assertIn("promise by practice", self.faults({"promise": practice, "delivered": "no", "recommendation": "problem"})[0])

    def test_claims_that_are_not_sorted_and_recovery_questions_are_not_checked(self):
        self.assertEqual(self.faults({"recommendation": "duplicate"}, ""), [])
        self.assertEqual(self.faults({"kind": "recovery"}, ""), [])


if __name__ == "__main__":
    unittest.main()
