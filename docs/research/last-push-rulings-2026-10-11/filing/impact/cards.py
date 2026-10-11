#!/usr/bin/env python3
"""Write blinded impact cards for the known problems the last-push rulings added, joined or widened.

Usage: python3 docs/research/last-push-rulings-2026-10-11/filing/impact/cards.py OUT KEY
It follows the second pass's `cards.py`: each record under `records/` is rendered with `bench/tools/calibration.py`'s
card text, so a card reads exactly like a filed one. A joined or widened problem is its filed entry with the record's
text and the record's full card; a joined problem's retired entry is left out. OUT must be new or empty and KEY
outside it."""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / "bench/tools"))
import calibration  # noqa: E402
import current_grading as current  # noqa: E402

HERE = Path(__file__).resolve().parent
TEXT = ("title", "obligation", "trigger", "mechanism")


def main(out, key):
    plan = json.loads((HERE / "plan.json").read_text(encoding="utf-8"))
    documents = calibration.load(ROOT, str(current.CURRENT))
    problems, cards = calibration.impact_problems(documents, str(current.CURRENT), ROOT)
    if problems:
        sys.exit("\n".join(problems))
    targets = {reference["target"]: reference for reference in documents["reference"]["targets"]}
    for entry in plan["new"] + plan["changed"]:
        record = json.loads((HERE / "records" / entry["record"]).read_text(encoding="utf-8"))
        families = targets[entry["target"]]["families"]
        if entry in plan["new"]:
            decision = f"last-push-{entry['family']}"
            documents["adjudication"]["decisions"].append(
                {"id": decision, "status": "approved", "receipt": current.pin_file(ROOT / entry["ruling"], ROOT)})
            families.append({"id": entry["family"], **{name: record[name] for name in TEXT}, "eligibility": {"adjudication": decision}})
        else:
            next(family for family in families if family["id"] == entry["family"]).update({name: record[name] for name in TEXT})
            families[:] = [family for family in families if family["id"] != record.get("joined")]
        card = {"schema_version": 1, "family": entry["family"], "target": entry["target"], **record["card"],
                "evidence": [current.pin_file(ROOT / path, ROOT) for path in record["evidence"]]}
        current.validate_schema("impact-card", card)
        cards[entry["family"]] = card
    texts, labels = calibration.blinded_cards(documents, cards, ROOT)
    wanted = [entry["family"] for entry in plan["new"] + plan["changed"]]
    out, key = Path(out), Path(key)
    if key.exists() or (out.exists() and any(out.iterdir())) or out.resolve() in key.resolve().parents:
        sys.exit("cards need a new or empty directory and an unused key path outside it")
    out.mkdir(parents=True, exist_ok=True)
    for family in wanted:
        (out / f"{family}.md").write_text(texts[family], encoding="utf-8")
    key.write_text(json.dumps({family: labels[family] for family in wanted}, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {len(wanted)} blinded impact cards to {out}")


if __name__ == "__main__":
    main(*sys.argv[1:])
