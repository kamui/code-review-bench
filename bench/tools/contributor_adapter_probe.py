"""Drive installed OpenRouter adapters against a local mock without paid API calls."""
from __future__ import annotations

import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import threading
from unittest.mock import patch

import contributor_adapters as adapters
from contributor_common import ROOT, command, read, write

MODEL = "openai/gpt-6.1-sol"
REPORT = {"status": "complete", "verdict": "clear", "findings": []}
PRICES = {"prompt": "0.000003", "completion": "0.000005", "input_cache_read": "0.0000003"}


def mock_handler(attempt, requests):
    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            messages = json.dumps(body.get("messages", []))
            child = "LOCAL-CHILD-PROBE" in messages and "Local mock probe." not in messages
            parent_count = sum(not row["child"] for row in requests)
            requests.append({"child": child, "model": body.get("model"), "reasoning": body.get("reasoning"),
                             "packet_occurrences": messages.count("Local mock probe.")})
            functions = {tool["function"]["name"]: tool["function"] for tool in body.get("tools", [])}
            if not child and parent_count == 0:
                name = "Agent" if "Agent" in functions else "subagent"
                arguments = {"prompt": "LOCAL-CHILD-PROBE: Review source.txt and return a short report.",
                             "description": "Local child adapter probe"}
                if name == "subagent":
                    arguments["agent"] = "general"
                delta = self.tool(name, arguments, "local-child")
                finish = "tool_calls"
            elif not child and parent_count == 1:
                report = str(attempt / "clone-work/report/review.json")
                if "write" in functions:
                    delta = self.tool("write", {"path": report, "content": json.dumps(REPORT)}, "local-write")
                else:
                    text = "*** Begin Patch\n*** Add File: " + report + "\n+" + json.dumps(REPORT) + "\n*** End Patch"
                    delta = self.tool("patch", {"patchText": text}, "local-write")
                finish = "tool_calls"
            else:
                delta = {"role": "assistant", "content": "Local child complete." if child else "Local adapter probe complete."}
                finish = "stop"
            usage = {"prompt_tokens": 100, "completion_tokens": 20, "total_tokens": 120,
                     "prompt_tokens_details": {"cached_tokens": 10}, "completion_tokens_details": {"reasoning_tokens": 5}}
            identity = {"id": f"local-probe-{len(requests)}", "created": 1, "model": body["model"]}
            self.send_response(200)
            if body.get("stream"):
                self.send_header("Content-Type", "text/event-stream")
                self.end_headers()
                chunks = [{**identity, "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": delta, "finish_reason": None}]},
                          {**identity, "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": {}, "finish_reason": finish}], "usage": usage}]
                for chunk in chunks:
                    self.wfile.write(("data: " + json.dumps(chunk) + "\n\n").encode())
                self.wfile.write(b"data: [DONE]\n\n")
            else:
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                payload = {**identity, "object": "chat.completion", "choices": [{"index": 0, "message": delta, "finish_reason": finish}], "usage": usage}
                self.wfile.write(json.dumps(payload).encode())

        @staticmethod
        def tool(name, arguments, identifier):
            return {"role": "assistant", "tool_calls": [{"index": 0, "id": identifier, "type": "function",
                    "function": {"name": name, "arguments": json.dumps(arguments)}}]}

        def log_message(self, *_args):
            pass
    return Handler


def probe(harness, attempt):
    attempt.mkdir()
    clone = attempt / "clone"
    clone.mkdir()
    command(["git", "init", "-b", "review-head", str(clone)])
    command(["git", "-C", clone, "config", "user.name", "Contributor"])
    command(["git", "-C", clone, "config", "user.email", "contributor@example.test"])
    (clone / "source.txt").write_text("test source")
    command(["git", "-C", clone, "add", "."])
    command(["git", "-C", clone, "commit", "-m", "test: initialize no-charge probe"])
    packet = attempt / "packet.md"
    packet.write_text("Review source.txt. Local mock probe.")
    skill = attempt / "skill"
    skill.mkdir()
    (skill / "SKILL.md").write_text("Review source.txt.")
    (attempt / "invocation.md").write_text((ROOT / "bench/contributor-invocation.md").read_text())
    runtime = adapters.prepare(harness, read(ROOT / "bench/contributor-profiles.json")["harnesses"][harness])
    config = {"harness": harness, "model": MODEL, "effort": "high", "kind": "skill", "runtime": runtime,
              "policy": "No external model requests. Local mock endpoint only.", "rates": {"model": {"pricing": PRICES}}}
    requests = []
    server = ThreadingHTTPServer(("127.0.0.1", 0), mock_handler(attempt, requests))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    def local_credentials(client, home, env):
        env["OPENROUTER_API_KEY"] = "local-probe-dummy-key-no-account"
        url = f"http://127.0.0.1:{server.server_port}/v1"
        if client == "opencode":
            settings = json.loads(env["OPENCODE_CONFIG_CONTENT"])
            settings["provider"]["openrouter"]["options"] = {"baseURL": url}
            env["OPENCODE_CONFIG_CONTENT"] = json.dumps(settings)
        else:
            models = {"providers": {"openrouter": {"baseUrl": url, "api": "openai-completions",
                      "apiKey": "local-probe-dummy-key-no-account", "models": [{"id": MODEL, "name": "Local no-charge control",
                      "reasoning": True, "contextWindow": 128000, "maxTokens": 4096}]}}}
            write(home / ".pi/agent/models.json", models)
            control = read(attempt / "pi-control.json")
            control["models"] = models
            write(attempt / "pi-control.json", control)
        return []

    try:
        with patch.object(adapters, "credentials", side_effect=local_credentials):
            result = adapters.execute(config, attempt, {"merge_base": "main", "head": "review-head",
                                      "provisioning": {"allowance": "read only", "unavailable": "all tests"}}, packet, skill)
        usage, report = adapters.collect(config, attempt)
        assert result == 0 and report["parse_status"] == "empty"
        assert usage["metering_status"] == "complete" and usage["requests"] == 4, usage
        assert abs(usage["priced_total_usd"] - .001492) < 1e-12, usage
        assert len(requests) == 4, requests
        assert sum(row["child"] for row in requests) == 1, requests
        assert all(row["model"] == MODEL and row["reasoning"] == {"effort": "high"} for row in requests), requests
        assert all(row["packet_occurrences"] == (0 if row["child"] else 1) for row in requests), requests
        evidence = {"harness": harness, "runtime": runtime, "exit": result, "requests": requests,
                    "usage": usage, "observed": read(attempt / "observed.json"), "report": report}
        write(attempt / "probe-result.json", evidence, immutable=True)
        print(json.dumps(evidence), flush=True)
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--harness", choices=("pi", "opencode", "both"), default="both")
    args = parser.parse_args()
    root = args.out.resolve()
    root.mkdir(parents=True)
    for harness in ("pi", "opencode") if args.harness == "both" else (args.harness,):
        probe(harness, root / harness)


if __name__ == "__main__":
    main()
