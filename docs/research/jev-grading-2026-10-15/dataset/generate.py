#!/usr/bin/env python3
"""Rebuild a historical human-credit experiment without model or network calls."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "bench/tools"))
import current_grading
from normalize_review import render

CURRENT = "bench/grading/current/"
TRIAL = "docs/research/cohort-rebuild-2026-10-05/trial/"
DEVELOPMENT = {"i-requests-6667", "o-astro-16079", "r-base-ui-5460"}
FIELDS = ("file", "line_start", "line_end", "claim", "consequence", "proposed_fix")
REF_FIELDS = ("title", "obligation", "trigger", "mechanism", "consequence")


def encoded(value):
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(value, message):
    if not value:
        raise ValueError(message)


def build():
    sources = {}
    unavailable_secondary = {}

    def pin(path):
        path = Path(path)
        path = path if path.is_absolute() else ROOT / path
        name = path.relative_to(ROOT).as_posix()
        result = {"path": name, "sha256": digest(path.read_bytes())}
        sources[name] = result
        return result

    def read(path):
        pin(path)
        return json.loads((ROOT / path).read_text())

    def verify(saved):
        require(pin(saved["path"]) == saved, f"Changed pinned source: {saved['path']}")

    credits = read(CURRENT + "credits.json")["rulings"]
    references = read(CURRENT + "references.json")["targets"]
    inventory = read(CURRENT + "inventory.json")
    claims = read(CURRENT + "claims.json")["claims"]
    decisions = {d["id"]: d for d in read(CURRENT + "adjudications.json")["decisions"]}
    policy = read(CURRENT + "validation-policy.json")
    trial = read(TRIAL + "ruled-comments.json")["comments"]
    pin(TRIAL + "README.md")
    pin("docs/clean-context.md")
    for path in ("bench/tools/current_grading.py", "bench/tools/normalize_review.py"):
        pin(path)
    for field in ("rubric", "rules", "grader"):
        verify(policy[field])

    rubric_path = ROOT / policy["rubric"]["path"]
    rubric_text = rubric_path.read_text()
    lines = rubric_text.splitlines(keepends=True)
    start = lines.index("## 3. Known problems\n")
    end = lines.index("## 4. Decisions already made\n")
    # Nested illustrative examples repeat some of the experimental answers.
    selected = [n for n in range(start + 1, end) if not lines[n].startswith("  ")]
    ordinary = "".join(lines[n] for n in selected).strip()
    pre_start = lines.index("## 6. Pre-existing problems\n")
    pre_existing = "".join(lines[pre_start + 1:]).strip()
    rubric = {"known_problems": ordinary, "pre_existing": pre_existing}
    rubric_selection = {
        "source": policy["rubric"],
        "known_problems_source_lines": [n + 1 for n in selected],
        "pre_existing_source_lines": list(range(pre_start + 2, len(lines) + 1)),
        "selection": "Section 3 normative paragraphs and top-level bullets, excluding indented illustrative examples; section 6 in full. No policy text is rewritten.",
    }

    targets = {t["id"]: t for t in inventory["tasks"]}
    ref_targets = {t["target"]: t for t in references}
    equivalent = {}
    for claim in claims:
        for link in claim["links"]:
            if link["relation"] == "equivalent":
                key = (link["review"]["path"], link["item_id"])
                require(key not in equivalent, "Multiple equivalent claims for one item")
                equivalent[key] = claim

    pairs = {}
    key_rows = []
    inputs = {}
    exposure_keys = {(f"bench/runs/{t['attempt'].split('/')[0]}/attempts/{t['attempt'].split('/')[1]}/normalized.json", f"item-{t['item'] - 1}") for t in trial}
    for ruling in credits:
        target = ruling["target"]
        subject = ruling.get("family_id", ruling.get("pre_existing_problem_id"))
        task, catalog = targets[target], ref_targets[target]
        require(ruling["revision"] == task["revision"] == catalog["revision"], "Ruling/reference/task revision mismatch")
        matches = [f for f in catalog["families"] + catalog.get("pre_existing_problems", []) if f["id"] == subject]
        require(len(matches) == 1, "Ruling does not identify exactly one current reference")
        reference = matches[0]
        verify(ruling["review"])
        verify(ruling["receipt"])
        receipt = (ROOT / ruling["receipt"]["path"]).read_text()
        require(ruling["receipt_scope"] in receipt, "Human receipt scope absent")
        require(ruling.get("settled_by", "owner") == "owner", "Only owner credit rulings are admitted")
        cohort, item = current_grading.source_item(ruling, target, ROOT)
        require(all(cohort[k] == ruling["revision"][k] for k in ("packet_sha256", "diff_manifest_sha256")), "Saved review uses another packet or diff")
        review_path = Path(ruling["review"]["path"])
        for path in (review_path.parent / "attempt.json", review_path.parents[2] / "manifest.json"):
            pin(path)
        verify(task["target"])
        verify(task["packet"])
        target_doc = json.loads((ROOT / task["target"]["path"]).read_text())
        require(all(target_doc[k] == ruling["revision"][k] for k in ("head", "base_sha")), "Pinned target uses another revision")
        for evidence in reference.get("evidence", []):
            if (ROOT / evidence["path"]).is_file():
                verify(evidence)
            else:
                unavailable_secondary[evidence["path"]] = evidence
        decision_id = reference.get("admission", reference.get("eligibility", {}).get("adjudication"))
        decision = decisions[decision_id]
        require(decision["status"] == "approved" and decision["revision"] == ruling["revision"], "Reference is not currently approved at this revision")
        verify(decision["receipt"])
        exact_item = {field: item[field] for field in FIELDS}
        require(all(isinstance(exact_item[k], (str, type(None))) for k in ("claim", "consequence", "proposed_fix")), "Unexpected item text")
        projected = {field: reference[field] for field in REF_FIELDS if field in reference}
        identity = digest(canonical({"comment": exact_item, "reference": projected}))
        case_id = "case-" + identity[:16]
        linked = equivalent.get((ruling["review"]["path"], ruling["item_id"]))
        grouping = linked["id"] if linked is not None else "exact:" + digest(canonical(exact_item))
        pair_key = (target, subject, grouping)
        labels = {axis: ruling[axis] for axis in ("says_what", "identifies_cause")}
        require(all(value in ("yes", "no", None) for value in labels.values()), "Unexpected human label")
        require(labels["says_what"] is not None, "Missing credit ruling")
        if pair_key in pairs:
            prior = pairs[pair_key]
            require(prior["expected"] == labels, "Equivalent items have conflicting human labels")
            prior["deduplicated_rulings"].append(ruling)
            continue
        partition = "development" if target in DEVELOPMENT else "qualification"
        caveats = []
        if subject == "GT-j1":
            caveats.append("The current receipt explicitly carries the old GT-j3 no-credit ruling into joined GT-j1. Only says_what is human-ruled; do not infer cause or transfer older positive labels.")
        if subject == "PE-j1":
            caveats.append("Activation ruling 14 explicitly carries this exact comment's no/yes ruling from GT-j5 into PE-j1 unchanged. Section 6 uses the same says_what test. No other old GT-j5 ruling is inferred.")
        private = {
            "case_id": case_id, "partition": partition, "target": target,
            "family_or_pre_existing": subject, "kind": "pre-existing" if "pre_existing_problem_id" in ruling else "known",
            "expected": labels, "metric_axes": [axis for axis, value in labels.items() if value is not None],
            "ruling": ruling, "deduplicated_rulings": [], "revision": ruling["revision"],
            "group": {"target": target, "family": subject, "equivalence": grouping},
            "raw_item_sha256": digest(canonical(item)),
            "canonical_original_render": render({"items": [item]}),
            "historical_trial_exact_item_overlap": (ruling["review"]["path"], ruling["item_id"]) in exposure_keys,
            "applicability": {"revision": "exact current target/catalog revision, pinned target head/base and original saved review packet/diff match", "receipt": "exact source pin and scope verified", "reference_approval": decision_id, "caveats": caveats},
        }
        pairs[pair_key] = private
        key_rows.append(private)
        inputs[case_id] = {"case_id": case_id, "comment": exact_item, "reference": projected, "rubric": rubric}

    key_rows.sort(key=lambda r: r["case_id"])
    input_rows = [inputs[row["case_id"]] for row in key_rows]
    require(len(inputs) == len(key_rows), "Case identifier collision")
    partitions = {name: [r["case_id"] for r in key_rows if r["partition"] == name] for name in ("development", "qualification")}
    split = {"schema_version": 1, "partitions": partitions, "unit": "whole target", "scope": "Held out from Jev input development only; all material is historically known."}

    # Historic trial answers do not automatically inherit today's reference scope.
    current_items = {(r["review"]["path"], r["item_id"]) for r in credits}
    candidates = []
    for old in trial:
        run, attempt = old["attempt"].split("/")
        path = f"bench/runs/{run}/attempts/{attempt}/normalized.json"
        item_id = f"item-{old['item'] - 1}"
        if (path, item_id) in current_items:
            continue
        pin(path)
        item = json.loads((ROOT / path).read_text())["items"][old["item"] - 1]
        candidates.append({"historical_trial": old, "source": sources[path], "item_id": item_id, "comment": {field: item[field] for field in FIELDS}, "status": "excluded-from-classifier", "reason": "Not an exact current credit ruling. Earlier trial labels and retired/widened family IDs need independent current applicability review; no model grade is ground truth."})

    counts = {
        "current_human_rulings": len(credits), "deduplicated_cases": len(key_rows),
        "unique_original_comments": len({(r["ruling"]["review"]["path"], r["ruling"]["item_id"]) for r in key_rows}),
        "targets": len({r["target"] for r in key_rows}), "target_family_groups": len({(r["target"], r["family_or_pre_existing"]) for r in key_rows}),
        "partitions": {name: len(ids) for name, ids in partitions.items()},
        "human_label_counts": {axis: dict(Counter(r["expected"][axis] or "unruled" for r in key_rows)) for axis in ("says_what", "identifies_cause")},
        "historical_trial_comments": len(trial), "historical_trial_exact_overlap": sum(r["historical_trial_exact_item_overlap"] for r in key_rows),
        "historical_candidates_without_current_ruling": len(candidates),
        "unavailable_secondary_reference_files": len(unavailable_secondary),
    }
    private_key = {
        "schema_version": 1, "purpose": "Private evaluation labels and source provenance; never send to a classifier or input-development assessor.",
        "counts": counts, "split": split, "split_sha256": digest(encoded(split)),
        "rubric_selection": rubric_selection, "cases": key_rows,
        "excluded_historical_candidates": candidates, "sources": sorted(sources.values(), key=lambda p: p["path"]),
        "unavailable_secondary_reference_evidence": sorted(unavailable_secondary.values(), key=lambda p: p["path"]),
        "source_limit": "Every primary classifier source, current ruling receipt, reference approval receipt, pinned revision/packet and available secondary reference file is hash checked. Missing unused secondary citations are recorded, not treated as verified. This experiment tests applicable saved human wording rulings, not a new adjudication of source-level truth.",
        "historical_exposure": "All cases are historical human-ruling material. Nine exact current source items occur in the older twenty-comment Opus/Sol rubric trial. Current rulings supersede old trial values. Other cases also informed later rubric edits. None is claimed fresh, and model grades supply no expected labels.",
    }
    def jsonl(rows):
        return b"".join(json.dumps(row, ensure_ascii=False, separators=(",", ":")).encode() + b"\n" for row in rows)

    artifacts = {"cases.jsonl": jsonl(input_rows), "split.json": encoded(split), "private-key.json": encoded(private_key)}
    for name, ids in partitions.items():
        artifacts[f"{name}.jsonl"] = jsonl([r for r in input_rows if r["case_id"] in ids])
    manifest = {
        "schema_version": 1, "experiment": "Historical paired credit-classifier proof of concept, not benchmark qualification",
        "generator": {"path": "generate.py", "sha256": digest(Path(__file__).read_bytes())},
        "split_sha256": digest(artifacts["split.json"]),
        "source_pins_sha256": digest(canonical(private_key["sources"])),
        "artifacts": [{"path": name, "sha256": digest(data)} for name, data in sorted(artifacts.items())],
        "classifier_inputs": ["development.jsonl", "qualification.jsonl"],
        "private_artifacts": ["private-key.json"],
        "counts": {"cases": len(key_rows), "development": len(partitions["development"]), "qualification": len(partitions["qualification"])},
        "source_contract": "Exact saved normalized claim/consequence/proposed_fix and location fields, selected current reference text, and verbatim normative rubric clauses. Original native metadata, priorities, reviewer identities, rulings, provenance and expected answers are excluded from classifier rows. Nested rubric examples are excluded because they repeat some case answers.",
        "limitations": "Small selected historical challenge set, not representative. Qualification is held out only from Jev input development. Unknown cause labels stay null in the private key and are omitted from cause metrics only. Current reference wording and policy were influenced by these human decisions.",
    }
    artifacts["manifest.json"] = encoded(manifest)
    return artifacts, counts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Verify exact regeneration without modifying any file")
    args = parser.parse_args()
    artifacts, counts = build()
    for name, data in artifacts.items():
        path = HERE / name
        if args.check:
            require(path.read_bytes() == data, f"Generated artifact differs: {name}")
        else:
            if name == "private-key.json":
                descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
                with os.fdopen(descriptor, "wb") as stream:
                    stream.write(data)
                path.chmod(0o600)
            else:
                path.write_bytes(data)
    print(json.dumps(counts, sort_keys=True))


if __name__ == "__main__":
    main()
