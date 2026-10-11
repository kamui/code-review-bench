#!/usr/bin/env python3
"""Turn the matchers' output into proposed links between the last-push reviews' items and the current claims.

    python3 docs/research/last-push-rulings-2026-10-11/links/collect.py PACKETS KEY

PACKETS is the directory `packets.py` wrote, now holding each matcher's `matches-N.json`; KEY is its key. The
result is `intake.v1.json` beside this file. It refuses output that does not cover every item of a part exactly
once, a match to an unknown claim, and a repeated match. Run from the repository root. It is the collector of
`docs/research/last-push-intake-2026-10-11/`, unchanged but for this note."""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
MATCHER = ("One fresh Codex GPT-6.1 Sol session at high reasoning effort per pull request. Each saw only its packets: the "
           "current claims' trigger, mechanism, consequence and relation to the change, the problems already recorded for the "
           "pull request, and every admitted item of the last-push reviews, with its file, lines, statement, consequence and "
           "proposed fix under an opaque id. No outcome, run, arm, attempt or model was in a packet.")
RELATION = {"only-claim": "equivalent", "with-other": "related"}


def main():
    packets, key = Path(sys.argv[1]), json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
    links, counts = [], {}
    for target, names in sorted(key.items()):
        seen = set()
        for part in sorted((packets / target).glob("part-*.json")):
            items = set(json.loads(part.read_text(encoding="utf-8"))["items"])
            result = json.loads((packets / target / part.name.replace("part", "matches")).read_text(encoding="utf-8"))
            matched = {row["item"] for row in result["matched"]}
            if matched & set(result["unmatched"]) or matched | set(result["unmatched"]) != items:
                raise SystemExit(f"{target} {part.name}: the output does not cover every item once")
            for row in result["matched"]:
                if row["claim"] not in names["claims"] or row["label"] not in RELATION:
                    raise SystemExit(f"{target} {part.name}: {row['item']} names an unknown claim or label")
                if (row["item"], row["claim"]) in seen:
                    raise SystemExit(f"{target}: {row['item']} is matched to {row['claim']} twice")
                seen.add((row["item"], row["claim"]))
                source = names["items"][row["item"]]
                links.append({"claim": names["claims"][row["claim"]], "target": target, "review": source["review"],
                              "attempt_id": source["attempt_id"], "item_id": source["item_id"],
                              "relation": RELATION[row["label"]], "reason": row["reason"]})
        counts[target] = {"claims": len(names["claims"]), "items": len(names["items"]),
                          "items_linked": len({item for item, _ in seen}),
                          "equivalent": sum(link["target"] == target and link["relation"] == "equivalent" for link in links),
                          "related": sum(link["target"] == target and link["relation"] == "related" for link in links)}
    (HERE / "intake.v1.json").write_text(json.dumps({"schema_version": 1, "matcher": MATCHER, "counts": counts, "links": links},
                                                    indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    print(len(links), "links")


if __name__ == "__main__":
    main()
