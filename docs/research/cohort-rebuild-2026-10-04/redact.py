#!/usr/bin/env python3
"""Replace the grading account's email address and account ids in archived grader transcripts.

Usage::

    python3 docs/research/cohort-rebuild-2026-10-04/redact.py --email ADDRESS --out RECORD RECEIPT [RECEIPT ...]

Each RECEIPT is the ``evidence.json`` that ``regrade.py`` wrote beside an ``evidence.tar.gz``. Claude Code puts
the signed-in account's email address and organization id into every session's context, so the saved
transcripts carry them; Codex puts the account's ``creator_user_id`` and ``creator_account_id`` into each session's
first line. The address is given on the command line and is never written to RECORD. Only ``.jsonl`` members change:
the address becomes ``[redacted-email]``, the value of ``organizationUuid`` becomes ``[redacted-organization]`` and
the two Codex ids become ``[redacted-account]``. The archive and its receipt are rewritten, and RECORD lists the earlier and
later hash of each archive, receipt and changed member. A receipt already redacted is left as it is.
"""

import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import tarfile

ROOT = Path(__file__).resolve().parents[3]
ORGANIZATION = re.compile(rb'("organizationUuid":")[0-9a-f-]{36}(")')
CODEX_ACCOUNT = re.compile(rb'("creator_(?:user|account)_id":")[^"]+(")')


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def redact(receipt_path, email):
    receipt_bytes = receipt_path.read_bytes()
    receipt = json.loads(receipt_bytes)
    if "redaction" in receipt:
        return None
    archive_path = ROOT / receipt["archive"]["path"]
    original = archive_path.read_bytes()
    members, changed = [], []
    with tarfile.open(fileobj=io.BytesIO(original), mode="r:gz") as bundle:
        for info in bundle:
            data = bundle.extractfile(info).read() if info.isfile() else None
            if data is not None and info.name.endswith(".jsonl"):
                clean = ORGANIZATION.sub(rb"\1[redacted-organization]\2", data.replace(email, b"[redacted-email]"))
                clean = CODEX_ACCOUNT.sub(rb"\1[redacted-account]\2", clean)
                if clean != data:
                    changed.append({"path": info.name, "before": sha256(data), "after": sha256(clean)})
                    data, info.size = clean, len(clean)
            members.append((info, data))
    if not changed:
        return None
    buffer = io.BytesIO()
    with gzip.GzipFile(fileobj=buffer, mode="wb", mtime=0) as compressed, tarfile.open(fileobj=compressed, mode="w") as bundle:
        for info, data in members:
            bundle.addfile(info, io.BytesIO(data) if data is not None else None)
    archive_path.write_bytes(buffer.getvalue())
    after = {row["path"]: row["after"] for row in changed}
    receipt["archive"]["sha256"] = sha256(buffer.getvalue())
    receipt["files"] = [{**row, "sha256": after.get(row["path"], row["sha256"])} for row in receipt["files"]]
    receipt["redaction"] = "Account email address and account ids replaced in the session transcripts."
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    return {"receipt": {"path": receipt_path.relative_to(ROOT).as_posix(), "before": sha256(receipt_bytes),
                        "after": sha256(receipt_path.read_bytes())},
            "archive": {"path": receipt["archive"]["path"], "before": sha256(original), "after": receipt["archive"]["sha256"]},
            "members": changed}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--email", required=True)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("receipts", nargs="+", type=Path)
    args = parser.parse_args()
    rows = [row for path in sorted(args.receipts) if (row := redact(path.resolve(), args.email.encode()))]
    with args.out.open("x", encoding="utf-8") as handle:
        handle.write(json.dumps({"schemaVersion": 1, "rule": "[redacted-email], [redacted-organization] and [redacted-account] "
                                 "replace the grading account's email address, organization id and Codex account ids "
                                 "in .jsonl members",
                                 "archives": rows}, indent=2) + "\n")
    print(f"redacted {len(rows)} archive(s), {sum(len(row['members']) for row in rows)} transcript(s)")


if __name__ == "__main__":
    main()
