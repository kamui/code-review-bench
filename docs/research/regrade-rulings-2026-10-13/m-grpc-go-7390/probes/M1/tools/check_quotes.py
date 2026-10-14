#!/usr/bin/env python3
"""Check every quotation of a Markdown file against the sources it was taken from.

    python3 check_quotes.py FILE.md DOSSIER_DIR HEAD_CHECKOUT TASK_PACKET

A quotation is a run of 20 or more characters between double quotes, or a line of a fenced code block. Each has
to appear character for character in one of: the saved upstream records and probe outputs under DOSSIER_DIR, the
task packet, or the source and documentation files of the head checkout. Leading indentation, a leading line
number and a leading diff marker are ignored for a code line. Lines of a `text` block are also tried joined, since
the probe's own result lines are quoted only in part. Prints what it could not find and exits 1 if there is any."""
import base64
import json
import re
import sys
from pathlib import Path

SOURCES = ("balancer/balancer.go", "balancer_wrapper.go", "clientconn.go", "connectivity/connectivity.go",
           "Documentation/versioning.md", "test/subconn_test.go", "balancer/pickfirst/pickfirst.go",
           "balancer/grpclb/grpclb_remote_balancer.go", "balancer/endpointsharding/endpointsharding.go",
           "xds/internal/balancer/outlierdetection/balancer.go", "interop/orcalb.go", "MAINTAINERS.md")


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


def corpus(dossier, head, packet):
    texts = [Path(packet).read_text(encoding="utf-8")]
    for name in SOURCES:
        texts.append((Path(head) / name).read_text(encoding="utf-8"))
    for path in sorted(Path(dossier).glob("upstream/*")) + sorted(Path(dossier).glob("probes/M1/**/*")):
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        texts.append(text)
        if path.suffix == ".json":
            loaded = json.loads(text)
            texts.extend(strings(loaded))
            if isinstance(loaded, dict) and loaded.get("encoding") == "base64":
                texts.append(base64.b64decode(loaded["content"]).decode("utf-8", "replace"))
    return "\n".join(texts).replace("\r\n", "\n")


def code_line(line, language):
    line = line.rstrip()
    if language == "diff":
        line = line[1:] if line[:1] in "+- " else line
    line = re.sub(r"^\s*\d+\t", "", line)
    return line.strip()


def main(file, dossier, head, packet):
    everything = corpus(dossier, head, packet)
    stripped = "\n".join(line.strip() for line in everything.splitlines())
    missing, checked, language = [], 0, None
    prose = []
    for line in Path(file).read_text(encoding="utf-8").splitlines():
        fence = re.match(r"\s*```(\w*)\s*$", line)
        if fence:
            language = None if language is not None else (fence.group(1) or "plain")
            continue
        if language is None:
            prose.append(line)
            continue
        wanted = code_line(line, language)
        if not wanted or wanted.startswith("@@"):
            continue
        checked += 1
        if wanted not in stripped:
            missing.append(f"code line: {wanted}")
    for quote in re.findall(r'"([^"\n]{20,})"', "\n".join(prose)):
        checked += 1
        if quote not in everything:
            missing.append(f"quotation: {quote}")
    print(f"{checked} quotations and code lines checked, {len(missing)} not found")
    for item in missing:
        print("  " + item)
    return 1 if missing else 0


if __name__ == "__main__":
    raise SystemExit(main(*sys.argv[1:5]))
