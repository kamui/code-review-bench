"""Snapshot pending entries and stage evidence in a contributor-owned release."""
from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import uuid

import contributor_common as common
from contributor_common import Refused, command, digest, file_index, lock, read, stamp, verify_index, write
import evidence_store
import contributor_secrets as secrets

BASE = "kamui/code-review-bench"


def gh(*arguments):
    return command(["gh", *arguments])


def api(route):
    return json.loads(gh("api", route))


def snapshot():
    import contributor
    journal = common.STATE / "submission.json"
    if journal.exists():
        saved = read(journal)
        if saved["status"] != "complete":
            excluded = any(not (common.STATE / "pending" / row["run_id"] / "entry.json").exists() for row in saved["entries"])
            if saved["status"] == "selected" and not saved.get("staging_repository") and excluded:
                saved["status"] = "abandoned-before-publication"
                write(common.STATE / "submissions" / saved["id"] / "journal.json", saved)
            else:
                return saved
    pending = common.STATE / "pending"
    selected = []
    for directory in sorted(pending.iterdir()) if pending.exists() else []:
        if not directory.is_dir() or directory.is_symlink() or not (directory / "entry.json").is_file():
            raise Refused(f"invalid pending entry {directory}; fix or exclude it before submitting")
        entry = read(directory / "entry.json")
        run = common.run_path(directory.name)
        expected = contributor.eligible(run, partial=entry["partial"])
        if entry != expected:
            raise Refused(f"pending entry {directory.name} changed or is invalid; inspect its run evidence")
        selected.append({"run_id": directory.name, "entry": entry, "entry_sha256": digest(directory / "entry.json")})
    if not selected:
        return None
    identifier = uuid.uuid4().hex
    saved = {"id": identifier, "status": "selected", "created_at": stamp(), "entries": selected,
             "branch": f"contributions/{identifier}", "tag": f"evidence-contribution-{identifier}"}
    write(journal, saved)
    return saved


def validate(saved):
    import contributor
    for row in saved["entries"]:
        run = common.run_path(row["run_id"])
        expected = contributor.eligible(run, partial=row["entry"]["partial"])
        if expected != row["entry"]:
            raise Refused(f"selected evidence changed for {run.name}; saved selection preserved at {common.STATE / 'submission.json'}")
        pending = common.STATE / "pending" / run.name / "entry.json"
        receipt = common.STATE / "submitted" / run.name / "receipt.json"
        if pending.exists() and digest(pending) != row["entry_sha256"]:
            raise Refused(f"pending entry changed during submission: {run.name}")
        if not pending.exists() and not receipt.exists():
            raise Refused(f"saved selection includes excluded {run.name}; restore its saved entry before retrying this submission")


def save(saved):
    write(common.STATE / "submission.json", saved)
    write(common.STATE / "submissions" / saved["id"] / "journal.json", saved)


def ensure_fork(saved):
    login = saved.get("login") or api("user")["login"]
    if login == BASE.split("/")[0]:
        raise Refused("use a contributor fork account for ./bin/bench submit; canonical release storage is maintainer-owned")
    repository = f"{login}/code-review-bench"
    try:
        fork = api(f"repos/{repository}")
    except Refused:
        gh("repo", "fork", BASE, "--clone=false", "--remote=false")
        fork = api(f"repos/{repository}")
    if not fork.get("fork") or fork.get("parent", {}).get("full_name") != BASE:
        raise Refused(f"{repository} is not a fork of {BASE}")
    saved.update(login=login, staging_repository=repository,
                 publication={"destination": f"{repository}/releases/tag/{saved['tag']}", "visibility": "public",
                              "categories": ["inputs", "execution", "accounting and lineage", "unique artifacts", "integrity"],
                              "authorized_at": stamp()})
    save(saved)


def package(saved):
    local = common.STATE / "submissions" / saved["id"]
    local.mkdir(parents=True, exist_ok=True)
    selection = []
    small = local / "records"
    if not small.exists():
        building = Path(tempfile.mkdtemp(prefix="records-building-", dir=local))
        for row in saved["entries"]:
            run = common.run_path(row["run_id"])
            destination = building / "bench/contributions" / saved["id"] / run.name
            destination.mkdir(parents=True)
            for name in ("manifest.json", "pending-created.json"):
                shutil.copy2(run / name, destination / name)
            write(destination / "entry.json", row["entry"], immutable=True)
            for attempt in (run / "attempts").iterdir():
                if attempt.name not in [r["id"] for r in row["entry"]["attempts"]]:
                    continue
                target = destination / "attempts" / attempt.name
                target.mkdir(parents=True)
                for name in ("attempt.json", "claim.json", "observed.json", "usage.json", "normalized.json", "workspace-pruned.json", "completion.json"):
                    if (attempt / name).is_file():
                        shutil.copy2(attempt / name, target / name)
        write(local / "records-index.json", {"files": file_index(building)}, immutable=True)
        building.rename(small)
    verify_index(small, read(local / "records-index.json")["files"])
    for row in saved["entries"]:
        run = common.run_path(row["run_id"])
        for item in file_index(run / "inputs"):
            selection.append({"source": str(run / "inputs" / item["path"]),
                              "path": f"artifacts/contributions/{saved['id']}/{run.name}/inputs/{item['path']}", "kind": "evidence"})
        for item in row["entry"]["attempts"]:
            attempt = run / "attempts" / item["id"]
            secrets.audit(attempt)
            for member in file_index(attempt, exclude=("clone", "clone-cache")):
                if Path(member["path"]).name in ("auth.json", ".credentials.json", ".claude.json"):
                    raise Refused(f"credential file found in selected evidence: {attempt / member['path']}; remove the credential copy before retrying")
                selection.append({"source": str(attempt / member["path"]),
                                  "path": f"artifacts/contributions/{saved['id']}/{run.name}/attempts/{item['id']}/{member['path']}", "kind": "evidence"})
    selection_doc = {"repository": saved["staging_repository"], "tag": saved["tag"],
                     "subjects": [row["run_id"] for row in saved["entries"]], "files": selection}
    packages = local / "packages"
    if not packages.exists():
        building = local / ("packages-building-" + uuid.uuid4().hex)
        evidence_store.pack(selection_doc, building)
        building.rename(packages)
    mapping = {"contract": "contributor-submission/v1", "submission": saved["id"],
               "staging_repository": saved["staging_repository"], "publication": saved.get("publication"),
               "runs": [{"run_id": row["run_id"], "entry": row["entry"],
                          "records": f"bench/contributions/{saved['id']}/{row['run_id']}",
                          "evidence_prefix": f"artifacts/contributions/{saved['id']}/{row['run_id']}"} for row in saved["entries"]],
               "storage_manifest": f"bench/evidence/manifests/contribution-{saved['id']}.json"}
    write(small / "bench/contributions" / saved["id"] / "submission.json", mapping, immutable=True)
    return local


def publish_assets(saved, local):
    try:
        release = api(f"repos/{saved['staging_repository']}/releases/tags/{saved['tag']}")
    except Refused:
        try:
            gh("release", "create", saved["tag"], "--repo", saved["staging_repository"], "--title",
               "Benchmark contribution evidence", "--notes", "Immutable ungraded review data staged for maintainer verification.")
        except Refused:
            # A write may have succeeded even when the response was lost.
            release = api(f"repos/{saved['staging_repository']}/releases/tags/{saved['tag']}")
        else:
            release = api(f"repos/{saved['staging_repository']}/releases/tags/{saved['tag']}")
    if release.get("draft"):
        raise Refused("staging release is a draft; publish it in your fork, then retry ./bin/bench submit")
    manifest_path = local / "records/bench/evidence/manifests" / f"contribution-{saved['id']}.json"
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    if not manifest_path.exists():
        evidence_store.publish(local / "packages/manifest.json", manifest_path)
    receipt_path = local / "records/bench/evidence/receipts" / f"contribution-{saved['id']}.json"
    if not receipt_path.exists():
        evidence_store.restoration_receipt(manifest_path, receipt_path)
    saved["status"] = "uploaded"
    saved["manifest"] = str(manifest_path.relative_to(local / "records"))
    save(saved)


def pr_body(saved):
    lines = ["## Why", "", "Contribute ungraded review data for maintainer verification. This submission does not change the scoreboard or adjudications.", "", "## Scope", ""]
    for row in saved["entries"]:
        entry = row["entry"]
        lines.append(f"- `{row['run_id']}`: {len(entry['completed'])} completed cells; {len(entry['missing'])} missing; partial={str(entry['partial']).lower()}.")
    lines += ["", "## Blast Radius", "", "Evidence and contribution records only. Large payloads are staged in the contributor fork and pinned by release asset IDs and hashes.", "", "## Verification", "", "- Revalidated frozen inputs, attempts, coverage and evidence hashes before submission.", "- Downloaded and restored every published evidence package before recording its manifest.", "", f"Staging: https://github.com/{saved['staging_repository']}/releases/tag/{saved['tag']}"]
    return "\n".join(lines) + "\n"


def push_records(saved, local):
    tree = local / "checkout"
    if not (tree / ".git").exists():
        if tree.exists():
            shutil.rmtree(tree)
        command(["git", "clone", "--no-hardlinks", "--no-checkout", str(common.ROOT), str(tree)])
    if not saved.get("head"):
        command(["git", "fetch", f"https://github.com/{BASE}.git", "main"], cwd=tree)
        command(["git", "checkout", "--force", "-B", saved["branch"], "FETCH_HEAD"], cwd=tree)
    elif command(["git", "rev-parse", "HEAD"], cwd=tree) != saved["head"]:
        raise Refused("submission checkout head changed; restore its saved commit before retrying")
    payload = local / "records"
    for path in payload.rglob("*"):
        if path.is_file():
            destination = tree / path.relative_to(payload)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, destination)
    command(["git", "add", "--", "bench/contributions", saved["manifest"], f"bench/evidence/receipts/contribution-{saved['id']}.json"], cwd=tree)
    dirty = command(["git", "status", "--porcelain"], cwd=tree)
    if dirty:
        command(["git", "commit", "-m", "data(bench): contribute ungraded review evidence"], cwd=tree)
    head = command(["git", "rev-parse", "HEAD"], cwd=tree)
    saved["head"] = head
    save(saved)
    command(["git", "push", f"https://github.com/{saved['staging_repository']}.git", f"HEAD:refs/heads/{saved['branch']}"], cwd=tree)


def find_pr(saved):
    rows = json.loads(gh("pr", "list", "--repo", BASE, "--state", "all", "--head", saved['branch'], "--base", "main",
                         "--json", "url,headRefOid,state,headRefName,headRepositoryOwner,headRepository"))
    prs = [row for row in rows if row["headRefName"] == saved["branch"]
           and row.get("headRepositoryOwner", {}).get("login") == saved["login"]
           and row.get("headRepository", {}).get("name") == saved["staging_repository"].split("/")[1]]
    if len(prs) > 1:
        raise Refused("multiple matching PRs found; inspect saved submission before retrying")
    if prs:
        if prs[0]["headRefOid"] != saved["head"]:
            raise Refused("existing PR head differs from the saved evidence commit")
        return prs[0]["url"]
    return None


def create_pr(saved, local):
    found = find_pr(saved)
    if found:
        return found
    body = local / "pr-body.md"
    body.write_text(pr_body(saved))
    try:
        result = gh("pr", "create", "--repo", BASE, "--head", f"{saved['login']}:{saved['branch']}", "--base", "main",
                    "--title", "data(bench): contribute review evidence", "--body-file", str(body))
    except Refused:
        found = find_pr(saved)
        if found:
            return found
        raise
    found = find_pr(saved)
    if not found or found not in result:
        raise Refused("PR response could not be reconciled; retry ./bin/bench submit to read the saved branch")
    return found


def finish(saved):
    for row in saved["entries"]:
        destination = common.STATE / "submitted" / row["run_id"]
        destination.mkdir(parents=True, exist_ok=True)
        receipt = {"pr_url": saved["pr_url"], "submission": saved["id"], "entry": row["entry"], "head": saved["head"]}
        path = destination / "receipt.json"
        if path.exists() and read(path) != receipt:
            raise Refused(f"conflicting submission receipt for {row['run_id']}")
        if not path.exists():
            write(path, receipt, immutable=True)
        pending = common.STATE / "pending" / row["run_id"]
        if pending.exists():
            if digest(pending / "entry.json") != row["entry_sha256"]:
                raise Refused(f"pending entry changed; receipt saved but removal withheld for {row['run_id']}")
            (pending / "entry.json").unlink()
            pending.rmdir()
    saved["status"] = "complete"
    save(saved)


def submit(yes=False):
    with lock(common.STATE / "submit.lock"):
        saved = snapshot()
        if not saved:
            print("No pending entries. Nothing submitted.")
            return
        for row in saved["entries"]:
            manifest = read(common.run_path(row["run_id"]) / "manifest.json")
            print(f"Selected {row['run_id']}: {len(row['entry']['completed'])}/{len(manifest['cells'])} cells")
            for config in manifest["configurations"]:
                print(f"  {config['invocation']} | {config['harness']} | {config['model']} | {config['effort']}")
        validate(saved)
        if not yes:
            if not sys.stdin.isatty():
                raise Refused("selection frozen; rerun ./bin/bench submit --yes to authorize public fork release uploads and one evidence-only PR")
            if input("Upload these evidence categories to your public fork release and open one PR? Type submit: ") != "submit":
                print("Selection preserved; no upload authorized. Next: ./bin/bench submit")
                return
        if not saved.get("staging_repository"):
            ensure_fork(saved)
        local = package(saved)
        if not saved.get("pr_url"):
            publish_assets(saved, local)
            validate(saved)
            push_records(saved, local)
            saved["pr_url"] = create_pr(saved, local)
            save(saved)
        finish(saved)
        print(f"Submitted {len(saved['entries'])} run(s): {saved['pr_url']}. Evidence retained under {common.STATE / 'runs'}")
