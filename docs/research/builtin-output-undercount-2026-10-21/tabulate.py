"""Write the re-meter table and the usage lower-bound deviation from remeter.py's output.

usage: python3 tabulate.py --repo REPO --remeter remeter.jsonl --table remeter.tsv --deviation DEVIATION
"""
import argparse, csv, json

ISSUE = "https://github.com/kamui/code-review-bench/issues/289"
RECORDED_AT = "2026-10-21T21:00:00Z"
COLUMNS = ["run", "attempt", "probe", "arm", "cli", "published", "filed_status", "remetered_status", "cause",
           "unfinalized_requests", "filed_cost_usd", "metered_cost_usd", "metered_output", "client_output",
           "client_cost_usd", "client_source"]


def cause(row):
    completeness = row.get("completeness") or {}
    if row.get("remeter") != "ok" or completeness.get("complete"):
        return ""
    if completeness["unfinalized"]:
        return "unfinalized-requests"
    if completeness["gaps"] and not completeness["refusals"] and not completeness["repairs"]:
        return "unrecorded-side-request"
    return "other"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--remeter", required=True)
    parser.add_argument("--table", required=True)
    parser.add_argument("--deviation", required=True)
    args = parser.parse_args()
    with open(f"{args.repo}/bench/scoreboard.current.json") as handle:
        published = {(source["run"].removeprefix("runs/"), source["arm"])
                     for source in json.load(handle)["sources"]}
    with open(args.remeter) as handle:
        rows = [json.loads(line) for line in handle]
    table, attempts = [], []
    for row in rows:
        client = row.get("client") or {}
        status = row["remeter"] if row["remeter"] != "ok" else ("complete" if row["complete"] else "incomplete")
        entry = {"run": row["run"], "attempt": row["attempt"], "probe": row["probe"], "arm": row["arm"],
                 "cli": row["cli"], "published": not row["probe"] and (row["run"], row["arm"]) in published,
                 "filed_status": row["filed_status"], "remetered_status": status, "cause": cause(row),
                 "unfinalized_requests": len((row.get("completeness") or {}).get("unfinalized") or []),
                 "filed_cost_usd": row["filed_usd"], "metered_cost_usd": row.get("metered_usd"),
                 "metered_output": (row.get("total") or {}).get("output"), "client_output": client.get("output"),
                 "client_cost_usd": client.get("cost_usd"), "client_source": client.get("source")}
        table.append(entry)
        if row["filed_status"] == "complete" and status == "incomplete":
            if entry["cause"] == "other" or row["metered_usd"] != row["filed_usd"]:
                raise SystemExit(f"{row['run']}/{row['attempt']}: not a lower bound this deviation covers")
            attempts.append({key: entry[key] for key in (
                "run", "attempt", "probe", "arm", "cli", "filed_cost_usd", "remetered_status", "cause",
                "unfinalized_requests")})
    with open(args.table, "w", newline="") as handle:
        writer = csv.DictWriter(handle, COLUMNS, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        for entry in table:
            writer.writerow({key: "" if entry[key] is None else entry[key] for key in COLUMNS})
    deviation = {
        "schema_version": 1,
        "recorded_at": RECORDED_AT,
        "kind": "usage lower bounds",
        "issue": ISSUE,
        "approved_by": "the repository owner, by approving and merging the pull request that adds this file",
        "reason": ("Claude Code left output counts unfinalized for every request but the last in each subagent "
                   "transcript, and some sessions billed a side request that no transcript records. Filing metered "
                   "these attempts as complete. Their filed costs and output counts are therefore lower bounds."),
        "rule": ("Each attempt below was filed with metering_status complete and re-meters incomplete with the "
                 "current meter at the filed rates. The re-metered cost equals the filed cost for every one. The "
                 "explorer exports its filed cost as a lower bound and its output tokens as unknown. Attempt records, "
                 "usage requests and transcripts are unchanged."),
        "causes": {
            "unfinalized-requests": ("A transcript request has no stop reason, so its recorded output count is "
                                     "partial and the session totals repeat the partial count."),
            "unrecorded-side-request": ("The session totals bill a request, a small claude-haiku-4-5 call, that no "
                                        "transcript records."),
        },
        "method": ("docs/research/builtin-output-undercount-2026-10-21/remeter.py, then tabulate.py; the full table "
                   "is docs/research/builtin-output-undercount-2026-10-21/remeter.tsv"),
        "attempts": attempts,
    }
    with open(args.deviation, "w") as handle:
        json.dump(deviation, handle, indent=2)
        handle.write("\n")


main()
