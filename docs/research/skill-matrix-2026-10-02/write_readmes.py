#!/usr/bin/env python3
"""Write each 2026-10-02 skill-matrix run's README from its filed evidence.

Usage::

    python3 docs/research/skill-matrix-2026-10-02/write_readmes.py

The text states only what the manifest, attempt records and results files hold. Rerun it after a
run gains attempts or a results file; it replaces the README.
"""

import json
import statistics
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HOLD = {"codex": "The ChatGPT plan had about 12% of its weekly usage left, so this run is held until the plan resets on 2026-10-07.",
        "claude": "The remaining Claude plan usage was reserved for grading and publishing, so this run is held until the plan resets on 2026-10-03."}


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def seconds(record):
    timing = record["timing"]
    end = timing.get("completed_at") or timing.get("stopped_at")
    if not end:
        return None
    parse = lambda text: datetime.fromisoformat(text.replace("Z", "+00:00"))  # noqa: E731
    return (parse(end) - parse(timing["dispatched_at"])).total_seconds()


def main():
    for run in sorted((ROOT / "bench/runs").glob("2026-10-02-*")):
        manifest = read(run / "manifest.json")
        arms = [arm["id"] for arm in manifest["arms"]]
        cells, tasks = len(manifest["planned_cells"]), len(manifest["cohort"])
        records = [read(path) for path in sorted(run.glob("attempts/*/attempt.json"))]
        valid = [r for r in records if r["disposition"] == "valid completed"]
        valid_cells = {(r["cell"]["target"], r["cell"]["arm"], r["cell"]["replicate"]) for r in valid}
        failed = [r for r in records if r["disposition"] != "valid completed"]
        cost = sum(r["usage"].get("priced_total_usd") or 0 for r in records)
        lines = [f"# {run.name}", "",
                 f"{cells} planned cells: {tasks} PR tasks, three repetitions each, arm{'s' if len(arms) > 1 else ''} "
                 + " and ".join(f"`{arm}`" for arm in arms) + f". Client `{manifest['arms'][0]['expected_cli_version']}`, "
                 f"frozen at commit `{manifest['freeze_commit'][:8]}`.", "", manifest["deviations"][0]["what"], ""]
        if not records:
            provider = "codex" if arms[0].startswith("codex") else "claude"
            lines += ["No attempt was dispatched. " + HOLD[provider], ""]
        else:
            times = [seconds(r) for r in valid if seconds(r) is not None]
            lines += [f"{len(valid_cells)} of {cells} trials are valid after {len(records)} attempts. Review usage totals "
                      f"${cost:.6f} at list price for subscription usage"
                      + (f"; a valid review took a median of {statistics.median(times) / 60:.1f} minutes." if times else "."), ""]
            if failed:
                lines += ["Attempts that are not valid stay filed with their usage:", ""]
                lines += [f"- `{r['attempt_id']}` ({r['cell']['target']}, repetition {r['cell']['replicate']}"
                          + (f", replaces `{r['predecessor']}`" if r.get("predecessor") else "") + f"): {r['disposition'][:160]}"
                          for r in failed]
                lines.append("")
            results = sorted(run.glob("results.v*.json"))
            if results:
                rows = read(results[-1])["by_arm"]
                lines += [f"Blinded Claude Opus 5.5 High graded every filed review under rubric v2; see [`{results[-1].name}`]({results[-1].name}) "
                          "and the mappings under `scoring/`.", "",
                          "| Arm | Findings score | False findings | Valid reviews |", "| --- | ---: | ---: | ---: |"]
                lines += [f"| `{row['key']['arm']}` | {row['recall_attempt_level']:.3f} | {row['false_findings_raw']} | {row['valid_reviews']['count']} |"
                          for row in rows]
                lines.append("")
            elif len(valid_cells) < cells:
                provider = "codex" if arms[0].startswith("codex") else "claude"
                lines += ["The run is incomplete and ungraded. " + HOLD[provider], ""]
        deviations = sorted((run / "deviations").glob("*.json")) if (run / "deviations").is_dir() else []
        if deviations:
            lines += ["Run deviations: " + ", ".join(f"[`{d.name}`](deviations/{d.name})" for d in deviations) + ".", ""]
        lines += ["Runner deviations that apply to every run of this matrix, the claim intake, grading plans and the time and "
                  "cost audit are in [the research record](../../../docs/research/skill-matrix-2026-10-02/README.md).", ""]
        (run / "README.md").write_text("\n".join(lines), encoding="utf-8")
        print(run.name, f"{len(valid_cells)}/{cells}")


if __name__ == "__main__":
    main()
