#!/usr/bin/env python3
"""Synthetic paired judgments that must not lose a mixed finding or a changed remedy."""

import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import sys

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("calibration_compare", Path(__file__).with_name("compare.py"))
compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare)


def claim(identity, quote, assignment="defect:GT-u3", fix="sufficient"):
    return {"id": identity, "quote": quote, "assignment": assignment, "canonical_claim_id": None,
            "duplicate_group": None, "fix_sufficiency": fix, "candidate": None,
            "notes": "Source supports the reply regression.", "evidence": ["pinned source"],
            "assessment": {"support": "supported", "attribution": "introduced",
                           "reachability": "reachable", "materiality": "material"}}


def mapping(claims):
    return {"schema_version": 2, "run_id": "paired", "target": "u-grpc-go-6919", "register": {"version": 2},
            "rubric_version": 2, "rubric_sha256": "pinned",
            "attempts": [{"attempt_id": "att-010", "blind_token": "control-token",
                          "review_level": {"zero_recovery": False},
                          "items": [{"item_id": "item-0", "assignment": "defect:GT-u3",
                                     "fix_sufficiency": "sufficient", "priority_error": False,
                                     "duplicate_group": None, "notes": "reply recovery", "claims": claims}]}]}


class CompareTest(unittest.TestCase):
    def test_changed_decomposition_and_remedy_survive_randomized_ids(self):
        control = mapping([claim("c1", "request and reply contents disappear")])
        enriched = mapping([claim("random-923", "reply contents disappear", fix="partial"),
                            claim("random-114", "request contents disappear", "refuted", "n/a")])
        enriched["attempts"][0]["blind_token"] = "enriched-random-token"
        item = compare.compare_mappings(control, enriched)["reviews"][0]["items"][0]
        self.assertTrue(item["decomposition_changed"])
        unmatched = item["claims"]["unmatched"]
        self.assertEqual(len(unmatched), 3)
        enriched_claims = [c for group in unmatched for c in group["enriched"]]
        self.assertEqual({c["assignment"] for c in enriched_claims}, {"refuted", "defect:GT-u3"})
        self.assertEqual(next(c for c in enriched_claims if c["assignment"] == "defect:GT-u3")["fix_sufficiency"],
                         "partial")

    def test_same_quote_changed_fix_reason_and_evidence_are_fields(self):
        control = mapping([claim("c1", "reply contents disappear")])
        enriched = copy.deepcopy(control)
        changed = enriched["attempts"][0]["items"][0]["claims"][0]
        changed.update(id="new-random-id", fix_sufficiency="absent", notes="No remedy proposed.",
                       evidence=["different inspected source"])
        item = compare.compare_mappings(control, enriched)["reviews"][0]["items"][0]
        fields = {row["field"] for row in item["claims"]["changed"][0]["differences"]}
        self.assertEqual(fields, {"fix_sufficiency", "notes", "evidence"})
        self.assertFalse(item["decomposition_changed"])

    def test_random_labels_do_not_change_duplicate_membership(self):
        control = mapping([claim("c1", "reply contents disappear"), claim("c2", "reply contents disappear")])
        for c in control["attempts"][0]["items"][0]["claims"]:
            c["duplicate_group"] = "control-token:group-1"
        enriched = copy.deepcopy(control)
        for index, c in enumerate(enriched["attempts"][0]["items"][0]["claims"]):
            c.update(id=f"random-{index}", duplicate_group="random-token:other-label")
        claims = compare.compare_mappings(control, enriched)["reviews"][0]["items"][0]["claims"]
        self.assertEqual(claims, {"changed": [], "unmatched": []})
        enriched["attempts"][0]["items"][0]["claims"][1]["duplicate_group"] = None
        claims = compare.compare_mappings(control, enriched)["reviews"][0]["items"][0]["claims"]
        self.assertEqual(claims["unmatched"][0]["reason"], "repeated quote is ambiguous")

    def test_status_reason_and_unresolved_claim_are_preserved(self):
        control = mapping([claim("c1", "reply contents disappear", "unresolved", "n/a")])
        control["attempts"][0]["items"][0]["claims"][0]["candidate"] = "candidate-1"
        enriched = mapping([claim("same-id", "reply contents disappear")])
        review = compare.compare_mappings(control, enriched)["reviews"][0]
        fields = {row["field"] for row in review["items"][0]["claims"]["changed"][0]["differences"]}
        self.assertIn("assignment", fields)
        self.assertIn("candidate", fields)
        self.assertEqual(len(review["outcomes"]["control"]["unresolved"]), 1)
        self.assertEqual(review["outcomes"]["enriched"]["unresolved"], [])

    def test_different_review_identity_is_refused(self):
        control = mapping([claim("c1", "reply contents disappear")])
        enriched = copy.deepcopy(control)
        enriched["attempts"][0]["attempt_id"] = "att-011"
        with self.assertRaisesRegex(ValueError, "review identities"):
            compare.compare_mappings(control, enriched)

    def test_missing_usage_is_unknown_and_repeated_receipt_is_refused(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "dispatch.json"
            path.write_text(json.dumps({"session_id": "session-1", "usage": {"priced_total_usd": None}}))
            summary = compare.usage_summary([path], Path(scratch))
            self.assertIsNone(summary["priced_total_usd"])
            self.assertEqual(summary["unpriced_attempts"], 1)
            with self.assertRaisesRegex(ValueError, "repeated dispatch session"):
                compare.usage_summary([path, path], Path(scratch))


if __name__ == "__main__":
    unittest.main()
