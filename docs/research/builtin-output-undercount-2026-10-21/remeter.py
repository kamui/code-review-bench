"""Re-meter every filed Claude attempt and probe with the current meter, the way filing selects transcripts.

Writes one JSON line per attempt. Reads each attempt's transcript archive by its frozen hash from the
given archive roots, or else its local workspace; extracts archives under the scratch directory. Where the
run directory (after `bun run evidence:fetch`) or the workspace kept the client's stream-json output, the
line also carries the client's exit totals and where they came from.

usage: python3 remeter.py --repo REPO --workspaces ~/.t3/bench-runs --archives DIR [--archives DIR ...]
                          --scratch DIR > remeter.jsonl
"""
import argparse, glob, hashlib, json, os, subprocess, sys, tarfile


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def rate(catalog, model, as_of):
    matches = [entry for entry in catalog["rates"] if entry["model"] == model and entry["as_of"] == as_of]
    return matches[0] if matches else None


def rate_for(repo, run, workspace, model, as_of):
    for path in (f"{repo}/bench/runs/{run}/rates.json", f"{workspace}/rates.json", f"{repo}/bench/rates.json",
                 f"{repo}/bench/rates.current.json"):
        if os.path.exists(path):
            with open(path) as handle:
                found = rate(json.load(handle), model, as_of)
            if found:
                return found
    return None


def api_errors_only(path):
    """Filing leaves out a transcript whose every assistant record is an API error notice."""
    assistant = []
    with open(path) as handle:
        for line in handle:
            if line.strip():
                record = json.loads(line)
                if record.get("type") == "assistant":
                    assistant.append(record)
    notice = lambda r: r.get("isApiErrorMessage") or (r.get("message") or {}).get("model") == "<synthetic>"
    return bool(assistant) and all(notice(r) for r in assistant)


def client_result(path):
    """The client's last stream-json result event, which holds its exit totals per model."""
    if not os.path.exists(path):
        return None
    last = None
    with open(path) as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if isinstance(event, dict) and event.get("type") == "result":
                last = event
    if not last or not last.get("modelUsage"):
        return None
    models = last["modelUsage"]
    total = lambda key: sum(entry.get(key, 0) for entry in models.values())
    return {"subtype": last.get("subtype"), "models": sorted(models), "input": total("inputTokens"),
            "cache_read": total("cacheReadInputTokens"), "cache_write": total("cacheCreationInputTokens"),
            "output": total("outputTokens"), "cost_usd": round(total("costUSD"), 6),
            "total_cost_usd": last.get("total_cost_usd")}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--workspaces", required=True)
    parser.add_argument("--archives", action="append", default=[])
    parser.add_argument("--scratch", required=True)
    args = parser.parse_args()
    repo = os.path.abspath(args.repo)
    bench_tools = [sys.executable, f"{repo}/bench/tools/rust_runtime.py", "exec", "bench-tools", "--"]
    archives = {}
    for root in args.archives:
        for path in sorted(glob.glob(f"{root}/**/*.tar.gz", recursive=True)):
            archives.setdefault(sha256(path), path)
    records = [(path, False) for path in sorted(glob.glob(f"{repo}/bench/runs/*/attempts/*/attempt.json"))]
    records += [(path, True) for path in sorted(glob.glob(f"{repo}/bench/runs/*/probes/*/attempt.json"))]
    for path, probe in records:
        with open(path) as handle:
            record = json.load(handle)
        observed = record.get("observed") or {}
        if observed.get("harness") != "claude-code":
            continue
        usage = record.get("usage") or {}
        run, attempt = path.split("/bench/runs/")[1].split("/")[0], path.split("/")[-2]
        key = f"{run}{'-probe' if probe else ''}/{attempt}"
        workspace = f"{args.workspaces}/{key}"
        arm = (record.get("cell") or {}).get("arm") or record.get("arm")
        arm_file = f"{repo}/bench/arms/{arm}.json"
        kind = None
        if arm and os.path.exists(arm_file):
            with open(arm_file) as handle:
                kind = json.load(handle)["kind"]
        row = {"run": run, "attempt": attempt, "probe": probe, "arm": arm, "kind": kind,
               "cli": observed.get("cli_version"), "models": observed.get("models"),
               "disposition": record.get("disposition"), "filed_status": usage.get("metering_status"),
               "filed_usd": usage.get("priced_total_usd"), "filed_low": (usage.get("cost_bounds_usd") or {}).get("low"),
               "rates_as_of": usage.get("rates_as_of")}
        for source in (f"{os.path.dirname(path)}/stdout.jsonl", f"{workspace}/stdout.jsonl"):
            row["client"] = client_result(source)
            if row["client"]:
                row["client"]["source"] = "run" if source.startswith(repo) else "workspace"
                break
        archive = archives.get((record.get("transcript_archive") or {}).get("sha256"))
        base, row["source"] = workspace, "workspace"
        if archive:
            base, row["source"] = f"{args.scratch}/{key}", "archive"
            if not os.path.isdir(base):
                os.makedirs(base)
                with tarfile.open(archive) as tar:
                    tar.extractall(base, filter="data")
        roots = sorted(glob.glob(f"{base}/home/.claude/projects/*/*.jsonl"))
        subs = sorted(glob.glob(f"{base}/home/.claude/projects/*/*/subagents/agent-*.jsonl"))
        models = observed.get("models") or []
        prices = rate_for(repo, run, workspace, models[0], usage.get("rates_as_of")) if len(models) == 1 else None
        if not (roots or subs) or prices is None:
            row["remeter"] = "unavailable: " + ("no transcripts" if not (roots or subs) else "no single filed rate")
            print(json.dumps(row), flush=True)
            continue
        everything = roots + subs
        meter = subs if kind == "claude-builtin" else everything
        unbilled = [each for each in meter if api_errors_only(each)]
        if unbilled and len(unbilled) < len(meter):
            meter = [each for each in meter if each not in unbilled]
        argv = bench_tools + ["usage", "claude", *meter, "--receipts", *everything,
                              "--prices", f"{prices['input']},{prices['output']}",
                              "--cache-read-mult", f"{prices['cache_read'] / prices['input']:g}",
                              "--cache-write-mult", f"{prices['cache_write_5m'] / prices['input']:g}",
                              "--cache-write-1h-mult", f"{prices['cache_write_1h'] / prices['input']:g}", "--json"]
        out = subprocess.run(argv, capture_output=True, text=True)
        try:
            result = json.loads(out.stdout)
        except ValueError:
            row["remeter"] = f"error {out.returncode}: {out.stderr.strip()[:300]}"
            print(json.dumps(row), flush=True)
            continue
        row["remeter"] = "ok"
        row["complete"] = result["completeness"]["complete"]
        row["metered_usd"] = result["total"]["cost"]
        row["completeness"] = result["completeness"]
        row["total"] = {k: result["total"][k] for k in ("input", "cache_write", "cache_read", "output", "turns")}
        print(json.dumps(row), flush=True)


main()
