from contextlib import contextmanager
from decimal import Decimal
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import URLError

import jev_runner as runner


class RunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.manifest = self.root / "manifest.json"
        self.ledger = self.root / "ledger"
        self.output = self.root / "output"
        self.body = {"model": "jev-1.13.0", "state": {"comment": "A failure description"},
                     "questions": {"failure": {"type": "choice", "instructions": "Does this identify the failure?",
                                                "criteria": {"yes": "It does", "no": "It does not", "cannot_tell": "Unclear"}}}}
        self.response = {"model": "jev-1.13.0", "usage": {"input_tokens": 1234, "output_tokens": 25},
                         "answers": {"failure": {"type": "choice", "choice": "yes",
                                                 "probabilities": {"yes": 0.8, "no": 0.1, "cannot_tell": 0.1},
                                                 "confidence": 0.6}}}
        self.secret = "test-key-never-save"
        self.env = patch.dict(os.environ, {"TYPESAFE_API_KEY": self.secret})
        self.env.start()
        self.addCleanup(self.env.stop)
        self.freeze()

    def freeze(self, mode="development"):
        data = runner.encode(self.body)
        (self.root / "request.json").write_bytes(data)
        self.manifest.write_bytes(runner.encode({"schema_version": 1, "mode": mode, "requests": [
            {"id": "case-v1", "path": "request.json", "sha256": runner.sha(data)}]}))
        self.pin = runner.sha(self.manifest.read_bytes())

    def run_request(self, transport=None, output=None):
        if transport is None:
            transport = lambda body, key: (200, runner.encode(self.response))
        return runner.run(self.manifest, self.pin, output or self.output,
                          ledger_directory=self.ledger, transport=transport)

    def assert_no_secret(self):
        for path in self.root.rglob("*"):
            if path.is_file():
                self.assertNotIn(self.secret.encode(), path.read_bytes(), str(path))

    def test_success_captures_frozen_bytes_usage_and_exact_cost(self):
        sent = []

        def transport(body, key):
            sent.append((body, key))
            self.assertTrue((self.output / "case-v1/home").is_dir())
            self.assertTrue((self.output / "case-v1/clean-context.json").is_file())
            return 200, runner.encode(self.response)

        summary = self.run_request(transport)
        self.assertEqual(sent, [((self.root / "request.json").read_bytes(), self.secret)])
        self.assertEqual(summary["accounting"]["actual_usd"], "0.000051828")
        self.assertEqual(Decimal(summary["accounting"]["unresolved_reserved_usd"]), Decimal(0))
        self.assertEqual(summary["results"][0]["usage"], {"input_tokens": 1234, "output_tokens": 25})
        self.assertEqual((self.output / "case-v1/response.json").read_bytes(), runner.encode(self.response))
        self.assertEqual(json.loads((self.output / "summary.json").read_bytes()), summary)
        self.assert_no_secret()

    def test_repeat_is_blocked_even_under_another_mode_and_output(self):
        self.run_request()
        self.freeze(mode="qualification")
        with self.assertRaisesRegex(runner.Refusal, "already attempted"):
            self.run_request(lambda *_: self.fail("retry dispatched"), self.root / "another-output")
        self.assertFalse((self.root / "another-output").exists())

    def test_service_rounding_is_accepted_but_invalid_mass_is_rejected(self):
        self.response["answers"]["failure"]["probabilities"] = {"yes": 0.67, "no": 0.27, "cannot_tell": 0.05}
        self.assertEqual(self.run_request()["results"][0]["status"], "completed")
        self.response["answers"]["failure"]["probabilities"]["yes"] = 0.6
        with self.assertRaisesRegex(runner.Refusal, "sum to one"):
            runner.validate_response(runner.encode(self.response), self.body["questions"])

    def test_noul_probability_is_captured_and_invalid_values_are_refused(self):
        question = self.body["questions"]["failure"]
        question.update(type="noul", criteria={"true": "Failure is stated", "false": "Failure is not stated"})
        self.response["answers"]["failure"] = {"type": "noul", "noul": 0.91}
        self.freeze()
        self.assertEqual(self.run_request()["results"][0]["status"], "completed")
        for invalid in (True, -0.1, 1.1, "0.91", None):
            with self.subTest(value=invalid):
                self.response["answers"]["failure"]["noul"] = invalid
                with self.assertRaises(runner.Refusal):
                    runner.validate_response(runner.encode(self.response), self.body["questions"])

    def test_network_failure_retains_reservation_and_blocks_retry(self):
        def transport(*_):
            raise URLError(self.secret)

        summary = self.run_request(transport)
        self.assertEqual(summary["results"][0]["status"], "unknown_charge")
        self.assertEqual(summary["accounting"]["unresolved_reserved_usd"], "0.005505024")
        self.assertEqual(Decimal(summary["accounting"]["actual_usd"]), Decimal(0))
        with self.assertRaisesRegex(runner.Refusal, "already attempted"):
            self.run_request(output=self.root / "retry")
        self.assert_no_secret()

    def test_wrong_model_and_invalid_response_keep_unknown_charge(self):
        for mutation in ("model", "probabilities", "usage", "confidence", "choice"):
            with self.subTest(mutation=mutation):
                self.ledger = self.root / ("ledger-" + mutation)
                self.output = self.root / ("output-" + mutation)
                invalid = json.loads(runner.encode(self.response))
                if mutation == "model":
                    invalid["model"] = "jev-1.14.0"
                elif mutation == "usage":
                    invalid["usage"]["input_tokens"] = True
                elif mutation == "choice":
                    invalid["answers"]["failure"]["choice"] = "no"
                else:
                    invalid["answers"]["failure"][mutation] = None
                summary = self.run_request(lambda *_: (200, runner.encode(invalid)))
                self.assertEqual(summary["results"][0]["status"], "unknown_charge")
                self.assertEqual(summary["accounting"]["committed_usd"], "0.005505024")

    def test_budget_refuses_before_transport(self):
        ledger = runner.Ledger(self.ledger)
        with ledger.locked():
            ledger.append({"event": "reserved", "request_sha256": "previous",
                           "reserved_usd": "4.949", "reservation_tokens": 131072})
        with self.assertRaisesRegex(runner.Refusal, "budget cannot cover"):
            self.run_request(lambda *_: self.fail("over-budget request dispatched"))
        self.assertEqual(ledger.accounting()["committed_usd"], "4.949")

    def test_lock_refuses_concurrent_dispatch(self):
        with runner.Ledger(self.ledger).locked():
            with self.assertRaisesRegex(runner.Refusal, "another runner"):
                self.run_request(lambda *_: self.fail("concurrent request dispatched"))
        self.assertFalse(self.output.exists())

    def test_missing_credential_and_hash_mismatch_do_not_create_output(self):
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(runner.Refusal, "credential environment variable is missing"):
                self.run_request(lambda *_: self.fail("uncredentialed request dispatched"))
        self.assertFalse(self.output.exists())
        (self.root / "request.json").write_text("{}")
        with self.assertRaisesRegex(runner.Refusal, "request hash mismatch"):
            self.run_request(lambda *_: self.fail("modified input dispatched"))
        self.assert_no_secret()

    def test_answer_key_and_oversized_request_refused(self):
        self.body["expected_answers"] = {"failure": "yes"}
        self.freeze()
        with self.assertRaisesRegex(runner.Refusal, "only state, model, questions"):
            self.run_request(lambda *_: self.fail("answer key dispatched"))
        del self.body["expected_answers"]
        self.body["state"] = "x" * 30001
        self.freeze()
        with self.assertRaisesRegex(runner.Refusal, "30000-byte context limit"):
            self.run_request(lambda *_: self.fail("oversized request dispatched"))

    def test_http_error_is_not_retried_or_released(self):
        calls = []

        def transport(*args):
            calls.append(args)
            return 429, b'{"error":"rate limited"}'

        summary = self.run_request(transport)
        self.assertEqual(len(calls), 1)
        self.assertEqual(summary["results"][0]["http_status"], 429)
        self.assertEqual(summary["accounting"]["unresolved_reserved_usd"], "0.005505024")

    def test_header_only_credential_and_no_long_context_header(self):
        @contextmanager
        def response():
            fake = io.BytesIO(b"{}")
            fake.status = 200
            yield fake

        class Opener:
            def open(inner, req, timeout):
                self.assertEqual(req.full_url, "https://api.typesafe.ai/v1/systemone")
                self.assertEqual(req.method, "POST")
                self.assertEqual(req.headers, {"Authorization": "Bearer " + self.secret,
                                              "Content-type": "application/json"})
                self.assertNotIn(self.secret.encode(), req.data)
                self.assertEqual(timeout, 120)
                return response()

        with patch.object(runner.request, "build_opener", return_value=Opener()):
            self.assertEqual(runner.post(b"{}", self.secret), (200, b"{}"))

    def test_response_echoing_secret_is_redacted_and_unresolved(self):
        summary = self.run_request(lambda *_: (200, self.secret.encode()))
        self.assertEqual(summary["results"][0]["status"], "unknown_charge")
        self.assertEqual((self.output / "case-v1/response.redacted").read_bytes(), b"[REDACTED]")
        self.assert_no_secret()


if __name__ == "__main__":
    unittest.main()
