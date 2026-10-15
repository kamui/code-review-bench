"""No model calls: exercise quota refusals, evidence integrity, isolation and cleanup."""

from argparse import Namespace
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import classify


def reading(percent=2, reset=classify.RESET):
    return {"codex": {"ordinaryUsageAllowed": True, "rateLimits": {
        "secondary": {"usedPercent": percent, "windowDurationMins": 10080, "resetsAt": reset}}}}


def packet():
    return {"model": "jev-1.13.0", "state": {"comment": "The path fails on empty input."},
            "questions": {"q": {"type": "choice", "instructions": "Does the comment describe a failure?",
                                  "criteria": {"yes": "Does", "no": "Does not", "cannot_tell": "Unclear"}}}}


class ClassifierTests(unittest.TestCase):
    def test_weekly_window_stops_at_launch_threshold_and_reset_change(self):
        self.assertEqual(classify.weekly(reading(54), 55), 54)
        for value in (reading(55), reading(60), reading(reset=0), reading(float("nan")), {}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                classify.weekly(value, 55)
        self.assertEqual(classify.weekly(reading(59), 60), 59)
        with self.assertRaises(ValueError):
            classify.weekly(reading(60), 60)

    def test_weekly_reset_tolerates_only_two_seconds_of_rounding_jitter(self):
        for delta in (-2, -1, 0, 1, 2):
            with self.subTest(delta=delta):
                self.assertEqual(classify.weekly(reading(5, classify.RESET + delta), 55), 5)
        for reset in (classify.RESET - 3, classify.RESET + 3, classify.RESET + 604800,
                      str(classify.RESET), float(classify.RESET), None):
            with self.subTest(reset=reset), self.assertRaises(ValueError):
                classify.weekly(reading(5, reset), 55)
        altered_window = reading(5, classify.RESET + 1)
        altered_window["codex"]["rateLimits"]["secondary"]["windowDurationMins"] = 10079
        with self.assertRaisesRegex(ValueError, "weekly quota window"):
            classify.weekly(altered_window, 55)

    def test_exact_question_ids_and_evidence_are_required(self):
        body = packet()
        answer = {"answers": {"q": {"choice": "yes", "reason": "The comment says it fails.",
                                     "evidence": [{"path": "/comment", "excerpt": "fails on empty input"}]}}}
        self.assertEqual(classify.validate_answer(json.dumps(answer).encode(), body), answer)
        answer["answers"]["q"]["evidence"][0]["excerpt"] = "invented source inspection"
        with self.assertRaises(ValueError):
            classify.validate_answer(json.dumps(answer).encode(), body)
        answer["answers"]["q"]["evidence"] = []
        with self.assertRaises(ValueError):
            classify.validate_answer(json.dumps(answer).encode(), body)
        answer["answers"]["q"]["choice"] = "cannot_tell"
        classify.validate_answer(json.dumps(answer).encode(), body)
        answer["answers"]["another-question"] = answer["answers"].pop("q")
        with self.assertRaises(ValueError):
            classify.validate_answer(json.dumps(answer).encode(), body)

    def test_choices_mode_requires_exact_choice_only_answers(self):
        body = packet()
        answer = {"answers": {"q": {"choice": "yes"}}}
        self.assertEqual(classify.validate_answer(json.dumps(answer).encode(), body, "choices"), answer)
        invalid = [
            {"answers": {"q": {"choice": "yes", "reason": "extra"}}},
            {"answers": {"q": {"choice": "yes", "evidence": []}}},
            {"answers": {"q": {"choice": "maybe"}}},
            {"answers": {"q": {"choice": ["yes"]}}},
            {"answers": {"q": {}}},
            {"answers": {}},
            {"answers": {"q": {"choice": "yes"}}, "reason": "extra"},
        ]
        for value in invalid:
            with self.subTest(value=value), self.assertRaises(ValueError):
                classify.validate_answer(json.dumps(value).encode(), body, "choices")
        with self.assertRaises(ValueError):
            classify.validate_answer(json.dumps(answer).encode(), body)

    def test_response_schemas_are_strict_and_packet_is_identical_across_modes(self):
        body = packet()
        for mode, fields in (("choices", {"choice"}), ("evidence", {"choice", "evidence", "reason"})):
            with self.subTest(mode=mode):
                schema = classify.schema(body["questions"], mode)
                answers = schema["properties"]["answers"]
                item = answers["properties"]["q"]
                self.assertFalse(schema["additionalProperties"])
                self.assertFalse(answers["additionalProperties"])
                self.assertFalse(item["additionalProperties"])
                self.assertEqual(set(item["properties"]), fields)
                self.assertEqual(set(item["required"]), fields)
                self.assertEqual(answers["required"], ["q"])
                self.assertEqual(set(item["properties"]["choice"]["enum"]), {"yes", "no", "cannot_tell"})
        marker = "\nFrozen packet (state and questions unchanged):\n"
        old_packet = classify.prompt(body).split(marker)[1]
        choices_packet = classify.prompt(body, "choices").split(marker)[1]
        self.assertEqual(old_packet, choices_packet)
        self.assertEqual(json.loads(choices_packet), {"state": body["state"], "questions": body["questions"]})
        self.assertNotIn("Evidence paths", classify.CHOICE_POLICY)
        self.assertNotIn("Keep rationales brief", classify.CHOICE_POLICY)
        self.assertIn("Use cannot_tell when the packet cannot establish an answer.", classify.CHOICE_POLICY)

    def test_examples_mode_changes_only_supplied_label_policy_and_keeps_strict_choices(self):
        self.assertEqual(hashlib.sha256(classify.POLICY.encode()).hexdigest(),
                         "ee9707aa56ff970917a8513b6c275e638504831ffe7cddfe022d2f0b0ed2f177")
        self.assertEqual(hashlib.sha256(classify.CHOICE_POLICY.encode()).hexdigest(),
                         "7aac9dcbd4a14631911bd64761f73a4c2fcb43637b173df6de8f02fe9c5f117e")
        self.assertIn("choices_examples", classify.RESPONSE_MODES)
        expected = classify.CHOICE_POLICY.replace(
            "No external tools, skills, memories, other graders, human labels, or repository context apply.",
            "Use only the supplied packet, including its supplied labelled examples. "
            "No external tools, skills, memories, other graders, unsupplied labels, or repository context apply.")
        self.assertEqual(classify.response_policy("choices_examples"), expected)
        body = packet()
        self.assertEqual(classify.schema(body["questions"], "choices_examples"),
                         classify.schema(body["questions"], "choices"))
        marker = "\nFrozen packet (state and questions unchanged):\n"
        self.assertEqual(classify.prompt(body, "choices_examples").split(marker)[1],
                         classify.prompt(body, "choices").split(marker)[1])
        answer = {"answers": {"q": {"choice": "yes"}}}
        self.assertEqual(classify.validate_answer(json.dumps(answer).encode(), body, "choices_examples"), answer)
        answer["answers"]["q"]["reason"] = "extra field"
        with self.assertRaises(ValueError):
            classify.validate_answer(json.dumps(answer).encode(), body, "choices_examples")

    def test_state_prefixed_pointer_validates_without_rewriting_raw_evidence(self):
        body = packet()
        answer = {"answers": {"q": {"choice": "yes", "reason": "The comment says it fails.",
                                     "evidence": [{"path": "/state/comment", "excerpt": "fails on empty input"}]}}}
        validated = classify.validate_answer(json.dumps(answer).encode(), body)
        self.assertEqual(validated, answer)
        self.assertEqual(validated["answers"]["q"]["evidence"][0]["path"], "/state/comment")
        answer["answers"]["q"]["evidence"][0]["excerpt"] = "absent excerpt"
        with self.assertRaisesRegex(ValueError, "excerpt not present"):
            classify.validate_answer(json.dumps(answer).encode(), body)

    def test_state_prefixed_pointer_does_not_repair_nonexistent_or_arbitrary_paths(self):
        state = packet()["state"]
        for path in ("/state/missing", "/packet/comment", "/state/state/comment", "/state"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                classify.pointer(state, path)

    def test_actual_state_key_keeps_original_pointer_resolution(self):
        state = {"state": {"comment": "nested"}, "comment": "outer"}
        self.assertEqual(classify.pointer(state, "/state/comment"), "nested")
        with self.assertRaises(ValueError):
            classify.pointer({"state": {}, "comment": "outer"}, "/state/comment")

    def test_config_contains_no_mcp_and_environment_drops_secrets(self):
        import tomllib
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            classify.clean_context.prepare(work)
            config = tomllib.loads(classify.configure(work, work, classify.MODELS[0]).read_text())
            self.assertNotIn("mcp_servers", config)
            self.assertEqual(config["sandbox_mode"], "read-only")
            self.assertFalse(config["features"]["shell_tool"])
            self.assertFalse(config["features"]["apps"])
            self.assertEqual(config["model_providers"]["classifier"]["request_max_retries"], 0)
            with patch.dict(os.environ, {"OPENAI_API_KEY": "never-forward", "GITHUB_TOKEN": "never-forward"}):
                env = classify.environment(work / "home", work / "tmp")
            self.assertEqual(set(env), {"PATH", "LANG", "HOME", "CODEX_HOME", "TMPDIR"})

    def run_fixture(self, directory):
        request = directory / "input.json"
        request.write_text(json.dumps(packet()))
        proof = directory / "proof.json"
        proof.write_text(json.dumps({"passed": True, "model": classify.MODELS[0],
                                     "response_mode": "evidence", "pins": {}, "advertised_tools": []}))
        return Namespace(request=request, request_sha256=classify.sha(request),
                         probe_receipt=proof, model=classify.MODELS[0], run_dir=directory / "attempt",
                         response_mode="evidence")

    def test_probe_mode_mismatch_is_refused_before_authentication_or_launch(self):
        for requested, proven in (("choices", "evidence"), ("evidence", "choices"), ("choices", None),
                                  ("choices_examples", "choices"), ("choices", "choices_examples"),
                                  ("choices_examples", "evidence"), ("evidence", "choices_examples")):
            with self.subTest(requested=requested, proven=proven), tempfile.TemporaryDirectory() as temporary:
                args = self.run_fixture(Path(temporary))
                args.response_mode = requested
                proof = json.loads(args.probe_receipt.read_text())
                if proven is None:
                    proof.pop("response_mode")
                else:
                    proof["response_mode"] = proven
                args.probe_receipt.write_text(json.dumps(proof))
                with patch.object(classify, "executable_path", return_value="codex"), \
                        patch.object(classify, "pins", return_value={}), \
                        patch.object(classify.codex_grade_dispatch, "preflight") as auth, \
                        patch.object(classify.subprocess, "Popen") as launch:
                    with self.assertRaisesRegex(ValueError, "response mode"):
                        classify.run(args)
                    auth.assert_not_called()
                    launch.assert_not_called()
                self.assertFalse(args.run_dir.exists())

    def test_bad_input_hash_never_reads_credentials_or_launches(self):
        with tempfile.TemporaryDirectory() as temporary:
            args = self.run_fixture(Path(temporary))
            args.request_sha256 = "bad"
            with patch.object(classify, "executable_path", return_value="codex"), \
                    patch.object(classify.codex_grade_dispatch, "preflight") as auth, \
                    patch.object(classify.subprocess, "Popen") as launch:
                with self.assertRaisesRegex(ValueError, "hash mismatch"):
                    classify.run(args)
                auth.assert_not_called()
                launch.assert_not_called()

    def test_noul_packet_is_refused_before_authentication_or_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            args = self.run_fixture(Path(temporary))
            body = packet()
            body["questions"]["q"]["type"] = "noul"
            body["questions"]["q"]["criteria"] = {"true": "Does", "false": "Does not"}
            args.request.write_text(json.dumps(body))
            args.request_sha256 = classify.sha(args.request)
            self.assertEqual(classify.jev_runner.validate_request(args.request.read_bytes()), body)
            with patch.object(classify, "executable_path", return_value="codex"), \
                    patch.object(classify.codex_grade_dispatch, "preflight") as auth, \
                    patch.object(classify.subprocess, "Popen") as launch:
                with self.assertRaisesRegex(ValueError, "only choice"):
                    classify.run(args)
                auth.assert_not_called()
                launch.assert_not_called()
            self.assertFalse(args.run_dir.exists())

    def test_luna_is_allowlisted_and_configured_at_high_effort(self):
        import tomllib
        self.assertIn("gpt-6-luna", classify.MODELS)
        catalog = json.loads(classify.codex_grading.MODEL_CATALOG.read_text())
        model = next(model for model in catalog["models"] if model["slug"] == "gpt-6-luna")
        self.assertIn("high", {level["effort"] for level in model["supported_reasoning_levels"]})
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            classify.clean_context.prepare(work)
            config = tomllib.loads(classify.configure(work, work, "gpt-6-luna").read_text())
            self.assertEqual(config["model"], "gpt-6-luna")
            self.assertEqual(config["model_reasoning_effort"], "high")

    def test_quota_refusal_keeps_evidence_and_never_copies_auth(self):
        with tempfile.TemporaryDirectory() as temporary:
            args = self.run_fixture(Path(temporary))
            with patch.object(classify, "executable_path", return_value="codex"), \
                    patch.object(classify, "pins", return_value={}), \
                    patch.object(classify.codex_grade_dispatch, "preflight"), \
                    patch.object(classify.clean_context, "neutral_directory", return_value=Path(tempfile.mkdtemp())), \
                    patch.object(classify, "meter", return_value=reading(55)), \
                    patch.object(classify.subprocess, "Popen") as launch:
                with self.assertRaisesRegex(ValueError, "55%"):
                    classify.run(args)
                launch.assert_not_called()
            self.assertFalse((args.run_dir / "home/.codex/auth.json").exists())
            self.assertTrue((args.run_dir / "clean-context.json").exists())
            self.assertTrue((args.run_dir / "meter-before.json").exists())
            receipt = json.loads((args.run_dir / "receipt.json").read_text())
            self.assertEqual(receipt["status"], "failed")
            self.assertTrue(receipt["credentials_removed"])

    def test_client_failure_strips_credentials_and_records_after_meter(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            args = self.run_fixture(directory)
            original = directory / "original"
            (original / ".codex").mkdir(parents=True)
            (original / ".codex/auth.json").write_text('{"auth_mode":"chatgpt","tokens":{"access_token":"secret"}}')
            process = Mock(returncode=1)
            process.poll.return_value = 1
            with patch.object(classify, "executable_path", return_value="codex"), \
                    patch.object(classify, "pins", return_value={}), \
                    patch.object(classify.codex_grade_dispatch, "preflight"), \
                    patch.object(classify.Path, "home", return_value=original), \
                    patch.object(classify.clean_context, "neutral_directory", return_value=Path(tempfile.mkdtemp())), \
                    patch.object(classify, "meter", return_value=reading()) as meters, \
                    patch.object(classify.subprocess, "Popen", return_value=process):
                with self.assertRaisesRegex(ValueError, "classification failed"):
                    classify.run(args)
                self.assertEqual(meters.call_count, 2)
            self.assertFalse((args.run_dir / "home/.codex/auth.json").exists())
            self.assertTrue((original / ".codex/auth.json").exists())
            self.assertTrue((args.run_dir / "meter-after.json").exists())
            self.assertNotIn("secret", (args.run_dir / "receipt.json").read_text())


if __name__ == "__main__":
    unittest.main()
