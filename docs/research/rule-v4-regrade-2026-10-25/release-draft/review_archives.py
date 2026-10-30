"""Credential and identity review of the grading archives, as the rule-v3 publication records describe it.

    python3 -I review_archives.py INVENTORY.json OUT.json [FILE_LIST]

INVENTORY.json is a list of {"path", "sha256"} rows (the selection's archives and evidence.json hashes). Every member of
each archive is read in memory; nothing is extracted or run. FILE_LIST optionally names loose files to scan as well.
OUT.json gets counts only; OUT.hits.json gets each pattern hit with surrounding text and stays local. The live values
of ~/.codex/auth.json and ~/.claude/.credentials.json are compared in memory and never written."""
import collections, hashlib, io, json, re, sys, tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
inv = json.loads(Path(sys.argv[1]).read_text()); out = Path(sys.argv[2])
PAT = {
 "private-key": rb"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----",
 "github-token": rb"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})",
 "openai-token": rb"\bsk-(?!ant-)(?:proj-|svcacct-|admin-)?[A-Za-z0-9_-]{20,}",
 "anthropic-token": rb"\bsk-ant-[A-Za-z0-9_-]{20,}",
 "bearer": rb"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{20,}",
 "aws-key": rb"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
 "slack-token": rb"\bxox[abposr]-[A-Za-z0-9-]{10,}",
 "jwt": rb"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
 "google-key": rb"\bAIza[0-9A-Za-z_-]{35}",
 "npm-token": rb"\bnpm_[A-Za-z0-9]{36}",
 "assigned-secret": rb"(?i)[\"']?\b(?:api[_-]?key|secret(?:[_-]?key)?|access[_-]?token|auth[_-]?token|refresh[_-]?token|id[_-]?token|password|passwd|client[_-]?secret|private[_-]?key)\b[\"']?\s*[:=]\s*[\"']?[A-Za-z0-9_\-/+=.]{16,}",
 "url-credential": rb"\b[a-z][a-z0-9+.-]*://[^/\s:@\"'<>]+:[^/\s@\"'<>]+@[A-Za-z0-9.-]+",
 # additions for this review
 "oauth-json-key": rb"\"(?:access_token|refresh_token|id_token|oauth_token|accessToken|refreshToken|idToken)\"\s*:\s*\"[^\"]{8,}",
 "google-oauth": rb"\bya29\.[A-Za-z0-9_-]{20,}|\b1//[A-Za-z0-9_-]{30,}",
 "anthropic-oauth": rb"\bsk-ant-o[a-z]t\d\d-[A-Za-z0-9_-]{20,}",
}
LOOSE = {"bearer-any": rb"(?i)\bbearer ", "sk-any": rb"\bsk-[A-Za-z0-9]", "gho-any": rb"\bgho_", "ghp-any": rb"\bghp_", "github_pat-any": rb"github_pat_"}
CRED_NAMES = {"auth.json", ".credentials.json", ".claude.json", "credentials", "credentials.json", ".netrc", ".npmrc",
              ".pypirc", "id_rsa", "id_ed25519", ".env", "hosts.yml", ".git-credentials", "config.toml",
              "installation_id", "models_cache.json"}
EMAILS = {"owner": rb"(?i)kamuigt@gmail\.com", "git-author": rb"(?i)jack@jackchu\.com"}
ANY_EMAIL = rb"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}"
HOME = rb"/home/jack\b"; WIN = rb"/mnt/c/Users/kamui\b"; WIN2 = rb"(?i)C:\\\\?Users\\\\?kamui"

def secret_values():
    vals = set()
    for f in [Path.home() / ".codex/auth.json", Path.home() / ".claude/.credentials.json"]:
        try:
            def walk(x):
                if isinstance(x, dict):
                    for v in x.values(): walk(v)
                elif isinstance(x, list):
                    for v in x: walk(v)
                elif isinstance(x, str) and len(x) >= 20: vals.add(x.encode())
            walk(json.loads(f.read_text()))
        except Exception as e:
            print("secret file unreadable", f.name, type(e).__name__)
    return vals
SECRETS = secret_values()
inst = (Path.home() / ".codex/installation_id").read_bytes().strip()
personal_markers = {"claude-agents-md": b"Make the smallest complete change that satisfies the request",
                    "memory-index": b"Release bench scratch promptly", "host-installation-id": inst,
                    "userEmail-attachment": b"userEmail", "credential_org": b"credential_org"}

def kind(name):
    n = name
    if "/.codex/sessions/" in n: return "codex session transcript (work/home/.codex/sessions/**/rollout-*.jsonl)"
    if n.startswith("work/reviews/"): return "blinded review (work/reviews/blind-*.md)"
    if n.startswith("work/home/"): return "client home file (" + n + ")" if n.count("/") < 5 else "client home file (work/home/...)"
    if n.startswith("work/validator/"): return "validator input (" + n + ")"
    if n.startswith("work/"): return n
    return n

def classify_email(addr):
    a = addr.lower()
    if a == "kamuigt@gmail.com": return "owner"
    if a == "jack@jackchu.com": return "git-author"
    if "noreply" in a: return "noreply"
    if re.search(r"@(example|test|localhost|domain|email)\.|@example$", a): return "example"
    return "other"

rows = []; agg = collections.defaultdict(collections.Counter); agg_files = collections.defaultdict(set)
kinds = collections.Counter(); kind_bytes = collections.Counter(); email_where = collections.defaultdict(collections.Counter)
cred_hits = []; cred_names = []; ids = collections.defaultdict(collections.Counter); other_emails = collections.Counter()
total_members = 0; total_unc = 0; anomalies = []; secret_hits = 0; marker = collections.Counter(); marker_where = collections.defaultdict(collections.Counter)
def scan_bytes(label, name, data):
    global secret_hits
    k = kind(name)
    for e, rx in EMAILS.items():
        n = len(re.findall(rx, data))
        if n: agg["email:" + e][label] += n; email_where[e][k] += n
    for m in set(re.findall(ANY_EMAIL, data)):
        c = classify_email(m.decode("utf-8", "replace"))
        if c not in ("owner", "git-author"): other_emails[(c, m.decode("utf-8", "replace").lower())] += 1
    for p, rx in PAT.items():
        for mm in re.finditer(rx, data):
            s = mm.group(0)
            cred_hits.append({"archive": label, "member": name, "pattern": p, "sha12": hashlib.sha256(s).hexdigest()[:12],
                              "preview": s[:14].decode("utf-8", "replace"), "len": len(s),
                              "context": data[max(0, mm.start()-160):mm.end()+60].decode("utf-8", "replace")})
    for p, rx in LOOSE.items():
        n = len(re.findall(rx, data))
        if n: agg["loose:" + p][label] += n; email_where["loose:" + p][k] += n
    for s in SECRETS:
        if s in data: secret_hits += 1
    for mk, b in personal_markers.items():
        n = data.count(b)
        if n: marker[mk] += n; marker_where[mk][k] += n
    for lab, rx in (("home", HOME), ("winprofile", WIN), ("winprofile-backslash", WIN2)):
        n = len(re.findall(rx, data))
        if n: agg[lab][label] += n; agg_files[lab].add((label, name)); email_where[lab][k] += n
    for key in (b"creator_user_id", b"creator_account_id", b"plan_type"):
        for mm in re.finditer(rb'"' + key + rb'"\s*:\s*"([^"]+)"', data):
            ids[key.decode()][hashlib.sha256(mm.group(1)).hexdigest()[:16]] += 1

def scan_archive(label, path, expect_sha):
    global total_members, total_unc
    raw = Path(path).read_bytes()
    if expect_sha and hashlib.sha256(raw).hexdigest() != expect_sha: anomalies.append((label, "hash mismatch"))
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as t:
        names = []
        for m in t.getmembers():
            if not m.isfile(): anomalies.append((label, "non-regular member " + m.name + " " + str(m.type))); continue
            if m.name.startswith("/") or ".." in m.name.split("/"): anomalies.append((label, "unsafe path " + m.name))
            data = t.extractfile(m).read(); names.append(m.name)
            total_members += 1; total_unc += len(data)
            k = kind(m.name); kinds[k] += 1; kind_bytes[k] += len(data)
            if data[:2] == b"\x1f\x8b": anomalies.append((label, "nested gzip " + m.name))
            base = m.name.rsplit("/", 1)[-1]
            if base in CRED_NAMES: cred_names.append({"archive": label, "member": m.name})
            scan_bytes(label, m.name, data)
    return names

per = []
for r in inv:
    names = scan_archive(r["path"], ROOT / r["path"], r["sha256"])
    per.append({"path": r["path"], "members": len(names), "sessions": sum("/.codex/sessions/" in n for n in names)})
extra = []
if len(sys.argv) > 3:
    for line in Path(sys.argv[3]).read_text().split():
        p = Path(line)
        for f in ([p] if p.is_file() else sorted(p.rglob("*"))):
            if f.is_file() and not f.is_symlink():
                data = f.read_bytes(); name = str(f)
                if f.name in CRED_NAMES: cred_names.append({"archive": "extra", "member": name})
                scan_bytes("extra:" + str(p), name.replace(str(p) + "/", ""), data); extra.append(name)
res = {"archives": len(per), "members": total_members, "uncompressed_bytes": total_unc, "anomalies": anomalies,
       "kinds": dict(kinds), "kind_bytes": dict(kind_bytes),
       "email_archives": {k: len(v) for k, v in agg.items() if k.startswith("email:")},
       "email_counts": {k: sum(v.values()) for k, v in agg.items()},
       "where": {k: dict(v) for k, v in email_where.items()},
       "files_with": {k: len(v) for k, v in agg_files.items()},
       "cred_hits_by_pattern": dict(collections.Counter(h["pattern"] for h in cred_hits)),
       "cred_names": dict(collections.Counter(c["member"].rsplit("/", 1)[-1] for c in cred_names)),
       "ids": {k: dict(v) for k, v in ids.items()}, "secret_value_hits": secret_hits, "secret_values_checked": len(SECRETS),
       "markers": dict(marker), "marker_where": {k: dict(v) for k, v in marker_where.items()},
       "other_emails_by_class": dict(collections.Counter(c for c, _ in other_emails)),
       "other_email_distinct": len(other_emails), "per_archive": per, "extra_files": len(extra)}
out.write_text(json.dumps(res, indent=1))
(out.parent / (out.stem + ".hits.json")).write_text(json.dumps({"cred_hits": cred_hits, "cred_names": cred_names,
    "other_emails": [[c, a, n] for (c, a), n in other_emails.most_common()]}, indent=1))
print(json.dumps({k: v for k, v in res.items() if k not in ("per_archive", "kinds", "kind_bytes")}, indent=1)[:6000])
