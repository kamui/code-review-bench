"""Report observed review usage without grading saved outputs."""
from pathlib import Path
import json

RUN = Path(__file__).resolve().parent
ROOT = RUN.parents[2]
WORK = ROOT / ".local/selected-pr-bench" / RUN.name
RATES = {r["model"]: r for r in json.loads((RUN / "rates.json").read_text())["rates"]}


def status():
    attempts = []
    max_input = 0
    for cell_path in sorted(WORK.glob("att-*/cell.json")):
        cell = json.loads(cell_path.read_text())
        responses = {}
        for rollout in (cell_path.parent / "home/.codex/sessions").rglob("*.jsonl"):
            model = None
            for line in rollout.read_text().splitlines():
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    continue
                payload = event.get("payload") or {}
                if event.get("type") == "turn_context":
                    model = payload.get("model", model)
                if event.get("type") == "token_usage_record":
                    key = payload.get("response_id") or (str(rollout), event.get("ordinal"))
                    responses[key] = (model, payload["usage"])
        cost = 0
        for model, usage in responses.values():
            rate = RATES[model]
            inp = usage.get("input_tokens", 0)
            cached = usage.get("cached_input_tokens", 0)
            write = usage.get("cache_write_input_tokens", 0)
            output = usage.get("output_tokens", 0)
            assert cached + write <= inp
            max_input = max(max_input, inp)
            input_multiplier = 2 if inp > 272000 else 1
            output_multiplier = 1.5 if inp > 272000 else 1
            cost += ((inp - cached - write) * rate["input"] + cached * rate["cache_read"] + write * rate["cache_write_5m"]) * input_multiplier / 1e6
            cost += output * rate["output"] * output_multiplier / 1e6
        record_path = RUN / "attempts" / cell_path.parent.name / "attempt.json"
        record = json.loads(record_path.read_text()) if record_path.exists() else None
        if record:
            assert record["usage"]["metering_status"] == "complete"
            assert abs(record["usage"]["priced_total_usd"] - cost) < 0.00002, record_path
        attempts.append({"attempt": cell_path.parent.name, "cell": cell["cell"], "disposition": record["disposition"] if record else "in flight", "observed_usd": round(cost, 6), "requests": len(responses)})
    charges = sum(json.loads(line)["usd"] for line in (RUN / "charges.jsonl").read_text().splitlines())
    total = sum(a["observed_usd"] for a in attempts) + charges
    return {"attempts": attempts, "setup_usd": charges, "observed_total_usd": round(total, 6), "max_request_input_tokens": max_input, "budget_cap_usd": 300, "dispatch_stop_threshold_usd": 225, "stop_dispatch": total >= 225, "note": "Observed completed requests only. Stop dispatch at $225 to retain $75 for in-flight requests and later user-authorized grading. Prices are token list-price equivalents."}


if __name__ == "__main__":
    print(json.dumps(status(), indent=2))
