#!/usr/bin/env python3
"""Configure and probe Codex grading tools without paid model calls."""

from __future__ import annotations

import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import threading
import time

import clean_context
import grading_policy


MODEL = "gpt-6-luna"
TOOL_NAMES = {"mcp__grading__" + name for name in
              ("inspect", "run", "write_scratch", "write_verdicts", "validate")}
RESOURCE_TOOLS = {"list_mcp_resources", "list_mcp_resource_templates", "read_mcp_resource"}
AUX_TOOL_NAMES = RESOURCE_TOOLS
MODEL_CATALOG = Path(__file__).resolve().parents[1] / "harness/codex-grading-models.v1.json"
MODEL_CATALOG_VERSION = "0.160.0"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def configure_client(work: Path, protected=(), execution_policy=None) -> Path:
    config = clean_context.configure_codex(work / "home", work, policy_text=execution_policy)
    catalog_path = work / "home/.codex/grading-models.json"
    shutil.copyfile(MODEL_CATALOG, catalog_path)
    settings = [
        'model_catalog_json = ' + json.dumps(str(catalog_path)), 'web_search = "disabled"',
        'approval_policy = "never"', 'sandbox_mode = "read-only"',
        'agents.enabled = false', 'features.multi_agent_v2 = false', 'features.code_mode = false',
        'tools.experimental_request_user_input.enabled = false',
        *(f"features.{feature} = false" for feature in
          ("shell_tool", "unified_exec", "multi_agent", "view_image", "browser_use", "computer_use",
           "image_generation", "goals", "sleep_tool", "skill_search", "skill_mcp_dependency_install")),
        '[mcp_servers.grading]', 'command = ' + json.dumps(sys.executable),
        'args = ' + json.dumps([str(Path(grading_policy.__file__).resolve()), "--work", str(work),
                               "--policy", str(work / "command-policy.json"),
                               *(argument for path in protected for argument in ("--protected", str(Path(path).resolve())))]),
        'required = true',
        'default_tools_approval_mode = "approve"',
        'enabled_tools = ' + json.dumps(sorted(name.removeprefix("mcp__grading__") for name in TOOL_NAMES)),
    ]
    mcp = settings.index('[mcp_servers.grading]')
    config.write_text("\n".join(settings[:mcp]) + "\n" + config.read_text()
                      + "\n".join(settings[mcp:]) + "\n")
    return config


def response(index, exercise=None, tokens=1):
    item = {"type": "message", "id": f"msg_{index}", "status": "completed", "role": "assistant",
            "content": [{"type": "output_text", "text": "probe complete", "annotations": []}]}
    if exercise:
        name, arguments = exercise
        item = {"type": "function_call", "id": f"fc_{index}", "call_id": "probe-" + name,
                "name": name, "arguments": json.dumps(arguments), "status": "completed"}
        if name not in RESOURCE_TOOLS:
            item["namespace"] = "mcp__grading"
    document = {"id": f"resp_{index}", "object": "response", "created_at": 1, "status": "completed",
                "model": MODEL, "output": [item],
                "usage": {"input_tokens": tokens, "output_tokens": tokens, "total_tokens": tokens * 2,
                          "input_tokens_details": {"cached_tokens": 0},
                          "output_tokens_details": {"reasoning_tokens": 0}}}
    return [{"type": "response.created", "response": {**document, "status": "in_progress", "output": []}},
            {"type": "response.output_item.added", "output_index": 0, "item": item},
            {"type": "response.output_item.done", "output_index": 0, "item": item},
            {"type": "response.completed", "response": document}]


def probe(evidence_dir: Path | None = None) -> dict:
    executable = shutil.which("codex")
    require(executable is not None, "Codex client is not installed")
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="codex-grading-") as temporary:
        root = Path(temporary) if evidence_dir is None else Path(evidence_dir).resolve()
        if evidence_dir is not None:
            root.mkdir(parents=True, exist_ok=False)
        work = root / "work"
        work.mkdir()
        clean_context.prepare(work)
        home = work / "home"
        (root / "AGENTS.md").write_text("ANCESTOR-CODEX-GRADING-PROBE-MARKER")
        (work / "AGENTS.md").write_text("AMBIENT-CODEX-GRADING-PROBE-MARKER")
        (work / ".codex").mkdir()
        (work / ".codex/config.toml").write_text('developer_instructions = "PROJECT-CODEX-GRADING-PROBE-MARKER"')
        skill = work / ".agents/skills/probe/SKILL.md"
        skill.parent.mkdir(parents=True)
        skill.write_text("---\nname: probe\ndescription: SKILL-CODEX-GRADING-PROBE-MARKER\n---\n")
        (work / "clone").mkdir()
        (work / "clone/test.txt").write_text("focused inspection")
        grading_policy.probe(work)
        validator = work / "validator"
        (validator / "tools").mkdir(parents=True)
        (validator / "schema").mkdir()
        tools = Path(grading_policy.__file__).resolve().parent
        for name in ("grading_validation.py", "claim_grading.py", "check_manifest.py"):
            shutil.copyfile(tools / name, validator / "tools" / name)
        shutil.copyfile(tools.parent / "schema/graded-claim.schema.json", validator / "schema/graded-claim.schema.json")
        (validator / "inputs.json").write_text(json.dumps({"rubric_version": 1, "defect_ids": [],
                                                         "reviews": {"blind-probe": {"items": []}}}))
        policy = work / "command-policy.json"
        policy.write_text(json.dumps({"test_kind": "none", "private_go": False, "go_flags": "-mod=readonly", "once": False}))
        private = root / "protected.txt"
        private.write_text("PRIVATE-CODEX-GRADING-PROBE-MARKER")
        exercises = [("inspect", {"path": "clone/test.txt"}),
                     ("run", {"argv": ["cat", "test.txt"], "cwd": "clone"}),
                     ("write_scratch", {"path": "clone-work/probe.txt", "text": "scratch probe"}),
                     ("write_verdicts", {"text": json.dumps({"reviews": {"blind-probe": {"items": {}}}, "new_candidates": []})}),
                     ("validate", {}),
                     ("list_mcp_resources", {"server": "grading"}),
                     ("list_mcp_resource_templates", {"server": "grading"}),
                     ("read_mcp_resource", {"server": "grading", "uri": private.as_uri()})]
        requests = []

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                if self.path != "/v1/responses":
                    self.send_error(404)
                    return
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                requests.append(body)
                index = len(requests)
                events = response(index, exercises[index - 1] if index <= len(exercises) else None)
                payload = "".join("data: " + json.dumps(event) + "\n\n" for event in events).encode()
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

            def log_message(self, *args):
                pass

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        config = configure_client(work, protected=(private,))
        config.write_text(f'model_provider = "grading_probe"\nmodel = "{MODEL}"\n' + config.read_text()
                          + '\n[model_providers.grading_probe]\nname = "Local grading compatibility probe"\n'
                          + f'base_url = "http://127.0.0.1:{server.server_port}/v1"\n'
                          + 'env_key = "BENCH_CODEX_GRADING_PROBE_KEY"\nwire_api = "responses"\n'
                          + 'requires_openai_auth = false\nsupports_websockets = false\n'
                          + 'request_max_retries = 0\nstream_max_retries = 0\n')
        env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "LANG": "C.UTF-8", "HOME": str(home),
               "CODEX_HOME": str(home / ".codex"), "TMPDIR": str(work / "tmp"),
               "BENCH_CODEX_GRADING_PROBE_KEY": "local-probe-only"}
        try:
            version = subprocess.run([executable, "--version"], env=env, capture_output=True, text=True, timeout=15)
            result = subprocess.run([executable, "exec", "--skip-git-repo-check", "--ignore-rules", "--json", "-"],
                                    cwd=work, env=env, input="Exercise every supplied grading tool once.",
                                    capture_output=True, text=True, timeout=60)
        finally:
            server.shutdown()
            server.server_close()
            worker.join()
        (root / "requests.json").write_text(json.dumps(requests, indent=2) + "\n")
        (root / "stdout.jsonl").write_text(result.stdout)
        (root / "stderr.txt").write_text(result.stderr)
        require(version.returncode == 0, "cannot check the Codex client version")
        require(result.returncode == 0, "Codex compatibility probe failed; inspect stderr.txt")
        require(len(requests) == len(exercises) + 1, "client did not exercise every grading tool and resource helper")
        for request in requests:
            advertised = request.get("tools", [])
            namespaces = [tool for tool in advertised if tool.get("type") == "namespace"]
            require(len(namespaces) == 1 and namespaces[0]["name"] == "mcp__grading", "unexpected native tool namespaces")
            names = {"mcp__grading__" + tool["name"] for tool in namespaces[0]["tools"]}
            helpers = {tool.get("name") for tool in advertised if tool.get("type") != "namespace"}
            require(names == TOOL_NAMES and helpers == RESOURCE_TOOLS, "unexpected native or MCP tools")
            require(request.get("model") == MODEL, "client requested an unexpected model")
        context = json.dumps(requests)
        require("CODEX-GRADING-PROBE-MARKER" not in context, "ambient context was loaded")
        require("### Available skills\n- " not in context, "ambient skills were loaded")
        rollouts = list((home / ".codex/sessions").rglob("rollout-*.jsonl"))
        require(len(rollouts) == 1, "client did not save exactly one fresh grading session")
        require(all("CODEX-GRADING-PROBE-MARKER" not in path.read_text() for path in rollouts),
                "ambient context was loaded into the saved session")
        results = {item["call_id"]: item["output"] for item in requests[-1]["input"]
                   if item.get("type") == "function_call_output"}
        for name, _ in exercises:
            require("probe-" + name in results, "grading tool result missing: " + name)
            output = results["probe-" + name]
            text = output if isinstance(output, str) else "\n".join(block.get("text", "") for block in output)
            require("approval policy is never" not in text and "unsupported call" not in text,
                    "grading tool returned an error: " + name)
        for name in ("run", "validate"):
            require(json.loads(results["probe-" + name][-1]["text"])["exit_code"] == 0,
                    "grading tool failed: " + name)
        require("resources/read failed" in str(results["probe-read_mcp_resource"]).lower(),
                "native resource reader did not reject the private file")
        audit = [json.loads(line) for line in (work / "policy-audit.jsonl").read_text().splitlines()]
        require({row["tool"] for row in audit} == {name.removeprefix("mcp__grading__") for name in TOOL_NAMES}
                and all(row["status"] == "completed" for row in audit), "grading policy audit did not confirm all tools")
        require("focused inspection" in context, "inspection did not complete")
        require((work / "clone-work/probe.txt").read_text() == "scratch probe", "scratch writer failed")
        require((work / "verdicts.json").is_file(), "verdict writer failed")
        receipt = {"client": "codex", "cli_version": version.stdout.strip(), "tools": sorted(TOOL_NAMES),
                   "native_helpers": sorted(RESOURCE_TOOLS),
                   "native_resource_helpers_confined": True, "catalog_cli_version": MODEL_CATALOG_VERSION,
                   "catalog_sha256": hashlib.sha256(MODEL_CATALOG.read_bytes()).hexdigest(),
                   "config_sha256": hashlib.sha256(config.read_bytes()).hexdigest(),
                   "saved_session_markers_absent": True,
                   "ambient_markers_absent": True, "ambient_skills_absent": True, "all_tools_completed": True,
                   "inspection_completed": True, "fresh_home": True, "budget_enforced": False,
                   "dispatch_ready": True, "paid_calls": 0,
                   "elapsed_seconds": round(time.monotonic() - started, 3),
                   "logs": {name: str(root / name) for name in ("requests.json", "stdout.jsonl", "stderr.txt")}
                   if evidence_dir is not None else {}}
        if evidence_dir is not None:
            (root / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        return receipt


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence-dir", type=Path, help="new directory to retain fake-API requests, logs and receipt")
    args = parser.parse_args()
    try:
        print(json.dumps(probe(args.evidence_dir)))
    except (RuntimeError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"Codex grading compatibility probe failed: {error}", file=sys.stderr)
        sys.exit(1)
