"""Earned-confidence routing and the common filing boundary for policy v2."""
from pathlib import Path
import re

import ruling_record as record

POLICY = "bench/grading/delegation.v2.json"
GATE = {"blind_answers": 2, "blind_confidence": "high", "recommender": "same-outcome",
        "recommender_confidence": "medium-or-high", "written": "before-question"}
ALWAYS_ASK = ["cannot-tell", "gap", "conflict", "relied-on", "problem", "reviews-saved-ruling"]
TYPES = {"recovery": "open", "candidate": "open", "reconciliation": "reconciler, 5 per audit round to the owner",
         "band": "owner", "grouping": "owner", "control": "owner"}
LEVELS = [{"level": 1, "decisions": 15, "pull_requests": 4, "owner_reads": "digest"},
          {"level": 2, "decisions": 30, "pull_requests": 5, "owner_reads": "1-in-5"}]


def policy(root, path=POLICY):
    source = root / path
    if not source.is_file():
        return None
    value = record.load(source)
    if (value.get("version") != 2 or value.get("gate") != GATE or value.get("always_ask") != ALWAYS_ASK
            or value.get("levels") != LEVELS or value.get("decision_types") != TYPES):
        raise ValueError("unsupported delegation policy; a changed rule needs a new version")
    if value.get("status") == "proposed" and value.get("receipt") is None:
        return None
    receipt = value.get("receipt")
    if (value.get("status") != "adopted" or not record.pinned(receipt, root)
            or not value.get("adoption_scope") or value["adoption_scope"] not in (root / receipt["path"]).read_text()):
        raise ValueError("policy v2 needs the saved adoption receipt and its verbatim scope")
    return value


def setup(before):
    return (record.recommender(before)["model"], tuple(sorted(answer["model"] for answer in before["answers"] if answer["blind"])))



def opening_rounds(current, pairs, root):
    pin = current.get("opening")
    if not record.pinned(pin, root) or current["before"].get("round_opening") != pin:
        raise ValueError("the before-record must pin its round's immutable opening snapshot")
    opening = record.load(root / pin["path"])
    if opening.get("round") != current["round"] or not re.fullmatch(r"[a-f0-9]{40}", opening.get("source_commit", "")):
        raise ValueError("round opening must name its round and the source commit")
    closed = set()
    for prior in opening["closed_rounds"]:
        if not record.pinned(prior, root):
            raise ValueError("a closed round pinned at opening changed")
        sheet = record.load(root / prior["path"])
        if sheet.get("closed") is not True or sheet["round"] == current["round"] or sheet["round"] in closed:
            raise ValueError("opening snapshot needs distinct earlier closed rounds")
        if any(pair["round"] == sheet["round"] and str(Path(pair["path"]).parent / "round.json") != prior["path"] for pair in pairs):
            raise ValueError("opening snapshot names a different round directory")
        closed.add(sheet["round"])
    return closed


def route(before, root=record.ROOT, policy_path=POLICY, pairs=None, before_path=None):
    import ruling_summary as summary

    adopted = policy(root, policy_path)
    if adopted is None:
        return {"level": 0, "reasons": ["no policy is adopted, so agents settle nothing beyond ADR-0006", *record.reasons(before, root)]}
    reasons = []
    if TYPES.get(before["decision_type"], "owner") != "open":
        reasons.append(f"{before['decision_type']} stays on its existing owner or reconciler route")
    if not summary.eligible({"before": before}):
        reasons.append("only contract 2 first answers written before the question qualify")
    if before["reviews"]:
        reasons.append("a review of a saved ruling always goes to the owner")
    for answer in before["answers"]:
        if answer["outcome"] in (*record.COULD_NOT_TELL, "relied-on", "problem"):
            reasons.append(f"{answer['by']}: {answer['outcome']} stays with the owner")
        if answer["rule_gap"] or answer["conflict"]:
            reasons.append(f"{answer['by']}: gap or conflict in the rule")
        if "cannot-tell" in (answer.get("facts") or {}).values():
            reasons.append(f"{answer['by']}: could not tell a credit fact")
    if not summary.consensus({"before": before}, root):
        reasons.append("two blind high-confidence answers and the medium-or-high recommender must agree")
    if pairs is None:
        pairs = summary.records(root)
    if before_path is not None:
        name = str(Path(before_path).resolve().relative_to(root.resolve()))
        current = next((pair for pair in pairs if pair["path"] == name), None)
    else:
        current = next((pair for pair in pairs if pair["before"] == before), None)
    if current is None:
        return {"level": 0, "reasons": [*reasons, "the before-record must belong to a recorded round"]}
    position = pairs.index(current)
    history = pairs[:position]
    if any(pair["before"]["decision_type"] == before["decision_type"] and pair["after"] is None for pair in history):
        reasons.append("an earlier decision of this type is still pending")
    try:
        closed = opening_rounds(current, pairs, root)
    except ValueError as error:
        return {"level": 0, "reasons": [*reasons, str(error)]}
    opening = summary.earned(pairs[:position + 1], before["decision_type"], root, current["round"], closed_rounds=closed)
    running = summary.earned(history, before["decision_type"], root)
    level = opening["level"]
    in_round = {pair["path"] for pair in history if pair["round"] == current["round"]}
    if any(reset["path"] in in_round for reset in running["resets"]):
        level = 0
        reasons.append("a miss or model change reset this decision type during the round")
    prior = next((pair for pair in reversed(history) if pair["before"]["decision_type"] == before["decision_type"]
                  and summary.eligible(pair)), None)
    if prior and setup(prior["before"]) != setup(before):
        level = 0
        reasons.append("a model changed; this decision type starts again")
    if not level:
        reasons.append("this decision type has not earned level 1 from earlier closed rounds")
    return {"level": level, "reasons": reasons}


def settlement_faults(before, after, root, before_path=None, pairs=None):
    if after.get("settled_by") != "agents":
        return []
    if policy(root) is None:
        return ["`settled_by` is `agents`, and the decision is the user's: no policy is adopted, so agents settle nothing beyond ADR-0006"]
    if before.get("contract") != 2 or after.get("contract") != 2:
        return ["agent settlement needs contract 2 before and after records"]
    pin = after.get("policy")
    if not record.pinned(pin, root) or pin["path"] != POLICY:
        return ["agent settlement must pin the policy v2 in force"]
    if before_path is not None and (root / after["before"]["path"]).resolve() != Path(before_path).resolve():
        return ["agent settlement pins a different before-record path"]
    routed = route(before, root, pairs=pairs, before_path=before_path)
    found = [f"agents cannot settle: {reason}" for reason in routed["reasons"]]
    first = record.recommender(before)
    for field in ("outcome", "same_fault_as", "facts"):
        if after.get(field) != first.get(field):
            found.append(f"agent settlement changes the agreed {field}")
    if after["asked"] != 0 or after["ground"] is not None or after["by_default"]:
        found.append("agent settlement has zero asks, no owner's ground and no default answer")
    return found


def filing_faults(decision, decision_type, root):
    """Current candidate and credit records use the same saved settlement and route."""
    settlement = decision.get("settlement")
    if decision.get("settled_by") != "agents":
        adopted = policy(root)
        if settlement or (adopted and decision.get("receipt") == adopted["receipt"]):
            return ["a settlement pin or policy adoption receipt requires settled_by agents"]
        return []
    if not record.pinned(settlement, root):
        return ["agent-settled current decision needs its pinned after-record"]
    after = record.load(root / settlement["path"])
    if not record.pinned(after.get("before"), root):
        return ["settlement needs its pinned before-record"]
    before_path = root / after["before"]["path"]
    before = record.read_before(before_path)
    found = record.faults(before, root)
    if found:
        return found
    found += record.after_faults(before, after, before_path, root)
    if after.get("settled_by") != "agents":
        found.append("the pinned record was not settled by agents")
    if before["decision_type"] != decision_type or before["target"] != decision["target"]:
        found.append("settlement is for another decision type or target")
    filing = before.get("filing")
    expected = {"revision": decision["revision"], "subject": decision.get("subject")} if decision_type == "candidate" else {
        key: decision[key] for key in ("revision", "review", "attempt_id", "item_id", "family_id")}
    if filing != expected:
        found.append("before-record filing does not bind this exact subject, revision and source")
    if decision_type == "candidate":
        mapped = {"suggestion": "advisory", "minor-defect": "inconsequential", "refuted": "refuted",
                  "unproven": "unsupported", "outside-scope": "scope-excluded", "duplicate": "eligible"}
        if (decision.get("dimension") != "eligibility" or decision.get("status") != "approved"
                or decision.get("authority") != "human" or mapped.get(after["outcome"]) != decision["outcome"]):
            found.append("candidate filing differs from the settled eligibility outcome")
    elif any(decision.get(field) != after.get("facts", {}).get(field) for field in record.FACTS):
        found.append("credit filing differs from the settled facts")
    adopted = policy(root)
    if adopted is None or decision["receipt"] != adopted["receipt"] or decision["receipt_scope"] != adopted["adoption_scope"]:
        found.append("agent settlement must cite the owner's saved policy adoption")
    if "policy v2" not in decision["reason"] or "agents" not in decision["reason"]:
        found.append("reason must say agents settled under policy v2")
    return found
