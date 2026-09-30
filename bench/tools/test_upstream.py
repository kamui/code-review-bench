"""Verify maintainer evidence cannot silently become claim eligibility."""

import copy
from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import check_manifest
import claims
import upstream


class Upstream(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.target = {"id": "t-example-1", "repo": "example/project", "pr": 1, "cutoff": "2026-01-01T00:00:00Z",
                       "head": "a" * 40, "base_sha": "b" * 40, "packet_sha256": "c" * 64,
                       "diff_manifest_sha256": "d" * 64}
        self.write("bench/targets/t-example-1/target.json", self.target)
        ep = upstream.endpoints(self.target["repo"], 1)
        self.snapshot = {"schema_version": 1, "target": self.target["id"], "repo": self.target["repo"], "pr": 1,
                         "revision": {k: self.target[k] for k in upstream.PIN_FIELDS}, "review_cutoff": self.target["cutoff"],
                         "retrieved_at": "2026-09-29T00:00:00Z", "coverage": [
                             {"endpoint": e, "complete": True, "pages": [[]], "error": None} for e in ep]}
        self.snapshot["coverage"][0]["pages"] = [{"id": 10, "number": 1, "base": {"repo": {"full_name": "example/project"}},
                                                "body": "PR merged", "user": {"login": "author", "type": "User"}}]
        self.snapshot["coverage"][1]["pages"] = [[{"id": 11, "body": "This is a bug, but defer the fix.",
                                                  "user": {"login": "maintainer", "type": "User"}},
                                                 {"id": 12, "body": "This is a bug.",
                                                  "user": {"login": "review[bot]", "type": "Bot"}}]]
        snapshot_ref = self.write("snapshot.json", self.snapshot)
        role_ref = self.write("role.json", {"path": "affected/file", "maintainer": "maintainer"})
        self.assessment = {"technical": {"status": "supported", "reason": "Pinned reproduction", "evidence": [snapshot_ref]},
                           "attribution": {"status": "introduced", "reason": "Base passes and head fails", "evidence": [snapshot_ref]},
                           "materiality": {"status": "material", "reason": "Functional failure", "evidence": [snapshot_ref]},
                           "maintainer": {"status": "unknown", "reason": "No matched ruling found", "snapshots": [snapshot_ref],
                                          "judgments": []}}
        self.judgment = {"snapshot": snapshot_ref, "endpoint": ep[1], "record_id": 11,
                         "quote": "This is a bug, but defer the fix.", "reason": "Same trigger and consequence",
                         "role": {"status": "confirmed", "reason": "Responsible for affected path", "evidence": [role_ref]},
                         "relation": "equivalent", "applicability": "present-at-head", "disposition": "deferred"}
        self.case = {"target": self.target["id"], "assessment": self.assessment, "decision": None}

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value))
        return claims.reference(path, self.root)

    def validate(self):
        problems = check_manifest.validate(claims.read(claims.ROOT / "bench/schema/claim.schema.json")["properties"]["assessment"],
                                           self.assessment)
        self.assertEqual(problems, [])
        upstream.validate_assessment(self.case, self.root, claims.resolve)

    def test_silence_and_merge_are_unknown_without_false_label_or_approval(self):
        self.validate()
        self.assertEqual(upstream.route(self.case), "shadow-proposal")
        self.assertIsNone(self.case["decision"])

    def test_valid_deferred_bug_is_not_a_false_finding(self):
        self.assessment["maintainer"].update(status="deferred", judgments=[self.judgment])
        self.validate()
        self.assertEqual(upstream.route(self.case), "shadow-proposal")
        self.case["decision"] = {"status": "approved", "outcome": "eligible"}
        self.assertEqual(upstream.route(self.case), "settled")

    def test_early_fixed_and_related_rulings_cannot_establish_disposition(self):
        for field, value in [("applicability", "fixed-before-head"), ("relation", "related")]:
            judgment = copy.deepcopy(self.judgment)
            judgment[field] = value
            self.assessment["maintainer"].update(status="deferred", judgments=[judgment])
            with self.assertRaisesRegex(ValueError, "matching responsible-human"):
                self.validate()
            self.assessment["maintainer"]["status"] = "unknown"
            self.validate()

    def test_bot_cannot_supply_responsible_human_authority(self):
        judgment = copy.deepcopy(self.judgment)
        judgment.update(record_id=12, quote="This is a bug.")
        self.assessment["maintainer"].update(status="deferred", judgments=[judgment])
        with self.assertRaisesRegex(ValueError, "bot or unidentified"):
            self.validate()

    def test_unverified_role_and_empty_judgments_cannot_establish_acceptance(self):
        self.assessment["maintainer"].update(status="accepted", judgments=[])
        with self.assertRaisesRegex(ValueError, "matching responsible-human"):
            self.validate()
        judgment = copy.deepcopy(self.judgment)
        judgment.update(disposition="accepted")
        judgment["role"]["status"] = "unverified"
        self.assessment["maintainer"]["judgments"] = [judgment]
        with self.assertRaisesRegex(ValueError, "matching responsible-human"):
            self.validate()

    def test_blinded_context_preserves_unknown_without_hiding_detection_credit(self):
        case = copy.deepcopy(claims.read(claims.ROOT / "bench/claims/CL-s-update-membership.v4.json"))
        text = claims.grading_context([case])
        self.assertIn("Approved outcome: eligible", text)
        self.assertIn("Maintainer disposition: unknown", text)
        self.assertIn("Detection credit does not require fix advice", text)
        self.assertNotIn("record_id", text)
        rendered, private = claims.dossier([case])
        self.assertIn("maintainer: unknown", rendered)
        self.assertTrue(any(k.startswith("assessment-") for k in private))

    def test_quote_and_record_must_match_archived_source(self):
        for field, value in [("record_id", 999), ("quote", "I reject this bug.")]:
            judgment = dict(self.judgment, **{field: value})
            self.assessment["maintainer"].update(status="deferred", judgments=[judgment])
            with self.assertRaisesRegex(ValueError, "not in the archived source"):
                self.validate()

    def test_modified_snapshot_and_wrong_revision_are_rejected(self):
        path = self.root / "snapshot.json"
        changed = copy.deepcopy(self.snapshot)
        changed["revision"]["head"] = "f" * 40
        path.write_text(json.dumps(changed))
        with self.assertRaisesRegex(ValueError, "source hash changed"):
            self.validate()
        ref = claims.reference(path, self.root)
        for name in ("technical", "attribution", "materiality"):
            self.assessment[name]["evidence"] = [ref]
        self.assessment["maintainer"]["snapshots"] = [ref]
        with self.assertRaisesRegex(ValueError, "revision changed"):
            self.validate()

    def test_conflicting_factual_rejection_routes_to_human(self):
        self.assessment["maintainer"]["status"] = "refuted"
        self.assertEqual(upstream.route(self.case), "human-conflict")

    def test_uncertainty_is_a_boundary_and_known_contradiction_is_a_conflict(self):
        self.case["decision"] = {"status": "approved", "outcome": "eligible"}
        self.assessment["technical"]["status"] = "unsettled"
        self.assertEqual(upstream.route(self.case), "human-boundary")
        self.assessment["technical"]["status"] = "refuted"
        self.assertEqual(upstream.route(self.case), "human-conflict")
        self.assessment["technical"]["status"] = "supported"
        self.case["decision"]["outcome"] = "non-material"
        self.assertEqual(upstream.route(self.case), "human-conflict")

    def test_incomplete_evidence_is_preserved_and_not_claimed_complete(self):
        failed = copy.deepcopy(self.snapshot)
        failed["coverage"][1].update(complete=False, pages=[], error="network unavailable")
        upstream.validate_snapshot(failed, self.target)
        failed["coverage"][1]["complete"] = True
        with self.assertRaisesRegex(ValueError, "retrieval coverage"):
            upstream.validate_snapshot(failed, self.target)

    def test_collection_is_get_only_and_preserves_pagination(self):
        pages = [[{"id": 1}], [{"id": 2}]]
        response = subprocess.CompletedProcess([], 0, stdout=json.dumps(pages), stderr="")
        with patch.object(upstream.subprocess, "run", return_value=response) as run:
            result = upstream.fetch("repos/example/project/pulls/1/comments")
        self.assertEqual(run.call_args.args[0][2:4], ["--method", "GET"])
        self.assertEqual(result["pages"], pages)

    def test_capture_keeps_failed_attempt_and_never_overwrites_snapshot(self):
        def failed(endpoint):
            return {"endpoint": endpoint, "complete": False, "pages": [], "error": "network unavailable"}
        with patch.object(upstream, "fetch", side_effect=failed), redirect_stdout(io.StringIO()):
            out = self.root / "upstream"
            self.assertFalse(upstream.capture(self.target["id"], [], out, self.root))
            first = (out / self.target["id"] / "snapshot.v1.json").read_bytes()
            self.assertFalse(upstream.capture(self.target["id"], [], out, self.root))
            self.assertEqual((out / self.target["id"] / "snapshot.v1.json").read_bytes(), first)
            self.assertTrue((out / self.target["id"] / "snapshot.v2.json").exists())


if __name__ == "__main__":
    unittest.main()
