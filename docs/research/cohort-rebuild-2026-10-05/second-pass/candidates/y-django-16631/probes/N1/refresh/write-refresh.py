import json
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[3]
out = root / "upstream"
clone = Path("<scratch>/y-django-16631/refresh-clone")


def save(name, data):
    with (out / f"refresh-N1-{name}.json").open("x") as stream:
        json.dump(data, stream, indent=2)


sections = {
    "docs/ref/settings.txt": [(2243, 2319)],
    "docs/howto/deployment/checklist.txt": [(42, 75)],
    "docs/topics/signing.txt": [(28, 40), (112, 122)],
    "docs/topics/auth/default.txt": [(909, 965)],
    "docs/topics/auth/customizing.txt": [(716, 731)],
    "docs/ref/contrib/auth.txt": [(686, 712)],
    "docs/topics/http/sessions.txt": [(300, 340)],
}
docs = []
for path, ranges in sections.items():
    lines = subprocess.check_output(["git", "-C", str(clone), "show", f"2396933ca99c6bfb53bda9e53968760316646e01:{path}"], text=True).splitlines()
    docs.append({"path": path, "sections": [{"first_line": start, "text": "\n".join(lines[start - 1:end])} for start, end in ranges]})
response = json.loads((out / "refresh-N1-project-docs.json").read_text())
save("project-docs-read", {"search": "upstream/refresh-N1-project-docs.json", "hits": response["hits"], "read": response["hits"], "sections_read": docs, "support_search": "upstream/refresh-N1-support-search.json"})

maintainer_queries = [
    {"query": 'gh api -X GET search/issues -f q=\'repo:django/django "SECRET_KEY_FALLBACKS" created:<=2023-03-08\' -f per_page=100', "saved": "upstream/refresh-N1-maintainers-fallbacks.json", "hits": 3, "read": 3},
    {"query": 'gh api -X GET search/issues -f q=\'repo:django/django "get_user" "rotation" created:<=2023-03-08\' -f per_page=100', "saved": "upstream/refresh-N1-maintainers-get-user.json", "hits": 0, "read": 0},
    {"query": 'gh api -X GET search/issues -f q=\'repo:django/django "secret key" "rolling" created:<=2023-03-08\' -f per_page=100', "saved": "upstream/refresh-N1-maintainers-rolling.json", "hits": 0, "read": 0},
    {"query": 'web search: site:code.djangoproject.com/ticket/ "SECRET_KEY_FALLBACKS" "rolling" before:2023-03-09; site:code.djangoproject.com/ticket/ "SECRET_KEY_FALLBACKS" "multiple" before:2023-03-09', "saved": "upstream/refresh-N1-trac-web-search.json", "hits": 2, "read": 2, "note": "Both results were later than cutoff. The date filter did not exclude them. Ticket 35400 and an attachment on ticket 37326 were read and excluded from the promise."},
]
save("maintainers-read", {"searches": maintainer_queries, "direct_search_failure": "upstream/refresh-N1-trac-direct-search.json", "replacement": "Indexed Trac search plus GitHub search; no required place remains blocked, but index coverage is limited.", "discussion_records_read": ["upstream/refresh-N1-pr-13850-comments.json", "upstream/pr-15198.json", "upstream/pr-15198-review-comments.json", "upstream/refresh-N1-pr-16631.json", "upstream/refresh-N1-pr-16631-comments.json", "upstream/pr-16631-issue-comments.json", "upstream/trac-30360.html", "upstream/trac-34384.html", "upstream/trac-35400.html"], "cutoff": "2023-03-08T09:48:04Z", "note": "Comments and events after cutoff cannot decide the promise. In particular PR 16631's 09:48:23 comment and ticket 35400 are later evidence."})

save("change-read", {"queries": ["git diff 9b224579875e30203d079cc2fee83b116d98eb78 2396933ca99c6bfb53bda9e53968760316646e01 -- django/contrib/auth docs tests/auth_tests/test_basic.py", "gh api repos/django/django/pulls/16631", "Read linked ticket 34384, saved earlier"], "hits": 8, "read": 8, "unit": "Six changed files, one PR description, one linked ticket", "records": ["upstream/refresh-N1-change.diff", "upstream/refresh-N1-pr-16631.json", "upstream/trac-34384.html"], "finding": "The change promises rotation with configured fallbacks and deliberately upgrades the stored hash. No mixed-server key-distribution guarantee is stated."})

save("public-code-read", {"queries": [
    {"query": 'gh api -X GET search/code -f q=\'"SECRET_KEY_FALLBACKS" "rolling" extension:py -repo:django/django\' -f per_page=100', "saved": "upstream/refresh-N1-public-rolling-retry.json", "hits": 7, "read": 7},
    {"query": 'gh api -X GET search/code -f q=\'"SECRET_KEY_FALLBACKS" "staged" extension:py -repo:django/django\' -f per_page=100', "saved": "upstream/refresh-N1-public-staged-retry.json", "hits": 4, "read": 4}], "dated_reads": "upstream/refresh-N1-code-read.json", "finding": "No returned program demonstrates the old-only/new-key server condition. All latest path changes are after cutoff and every pre-cutoff path-history query returned an empty list. These counts do not estimate deployment prevalence.", "initial_rate_limit_responses": ["upstream/refresh-N1-public-rolling.json", "upstream/refresh-N1-public-staged.json"]})

save("documented-way-read", {"commands": [
    "git checkout --quiet review-base; refresh-venv/bin/python probes/N1/probe.py refresh-clone",
    "git checkout --quiet review-head; refresh-venv/bin/python probes/N1/probe.py refresh-clone",
    "refresh-venv/bin/python probes/N1/refresh/signing.py refresh-clone"], "note": "Each git and Python command ran separately. Paths in the commands abbreviate the dossier and scratch roots; search.py and signing.py are saved under probes/N1/refresh.", "hits": 10, "read": 10, "unit": "Five session cases with each of two backends at each revision; the same ten cases were read at both revisions.", "results": ["probes/N1/refresh/result-base.txt", "probes/N1/refresh/result-head.txt", "probes/N1/refresh/result-signing.txt", "probes/N1/refresh/environment.txt"], "comparison_docs": "upstream/refresh-N1-itsdangerous-concepts.json", "deployment_context": "upstream/refresh-N1-kubernetes-deployment.json", "finding": "Uniform rotation and distributing both keys first preserve sessions at head. Old-only verification fails in Django auth, Django signing and ItsDangerous. Kubernetes documents rolling updates as the default, but no percentage of Django deployments with incompatible keys is established."})

refresh = [{
    "group": "N1",
    "promise": {
        "made_by": "none",
        "whose_interface": "Django get_user() and shared sessions: keeping a re-signed session usable on a server that lacks the signing key",
        "classification": "public",
        "source": None,
        "searched": {
            "project-docs": {
                "state": "checked",
                "query": "rg -n -i 'SECRET_KEY_FALLBACKS|rolling|multiple servers|multi.?server|secret.key.rotation|rotate.*secret' docs at head 2396933ca99c6bfb53bda9e53968760316646e01; read the general signing, deployment, settings, sessions and authentication sections",
                "saved": "upstream/refresh-N1-project-docs-read.json",
                "hits": response["hits"], "read": response["hits"],
                "found": "The signing guide says fallback values 'will not be used to sign data'. Settings and the deployment checklist document rotation with the old key as fallback. None promises that a server without the signing key accepts the upgraded session."
            },
            "owner-docs": {"state": "not-applicable", "reason": "Django owns authentication-hash verification and these session backends. No dependency owns the failing operation. ItsDangerous was tested only as a comparison."},
            "change": {
                "state": "checked", "query": "git diff 9b224579875e30203d079cc2fee83b116d98eb78 2396933ca99c6bfb53bda9e53968760316646e01 -- django/contrib/auth docs tests/auth_tests/test_basic.py; gh api repos/django/django/pulls/16631; read linked ticket 34384",
                "saved": "upstream/refresh-N1-change-read.json", "hits": 8, "read": 8,
                "found": "The added release note fixes invalidation during rotation with fallbacks. The auth reference promises checking configured fallback keys. The new test deliberately upgrades the hash so the old fallback can be removed. No mixed-server guarantee is stated."
            },
            "maintainers": {
                "state": "checked", "query": "GitHub issue search in django/django before 2023-03-09 for SECRET_KEY_FALLBACKS, get_user plus rotation, and secret key plus rolling; indexed Trac search for SECRET_KEY_FALLBACKS plus rolling and multiple. Exact queries and responses are saved.",
                "saved": "upstream/refresh-N1-maintainers-read.json", "hits": 5, "read": 5,
                "found": "Three GitHub hits and two later indexed Trac records were read. Pre-cutoff ticket 34384 accepts upgrading the stored hash. No pre-cutoff guarantee or refusal for mixed-server key sets was found. Direct Trac search was refused by robots.txt and replaced by indexed search; its coverage is limited."
            },
            "public-code": {
                "state": "checked", "query": "gh api -X GET search/code -f q='\"SECRET_KEY_FALLBACKS\" \"rolling\" extension:py -repo:django/django' -f per_page=100; repeat with staged instead of rolling; fetch each blob and dated path history",
                "saved": "upstream/refresh-N1-public-code-read.json", "hits": 11, "read": 11,
                "found": "All 11 returned Python files were read with dates. Some use fallbacks, but none demonstrates the claimed mixed-key deployment or pre-cutoff reliance. Every pre-cutoff path-history query returned no commits. Search counts do not establish prevalence."
            },
            "documented-way": {
                "state": "checked", "query": "Run the existing probes/N1/probe.py at base and head; run probes/N1/refresh/signing.py at head with old-only and prepared key sets",
                "saved": "upstream/refresh-N1-documented-way-read.json", "hits": 10, "read": 10,
                "found": "At head, uniform rotation and distributing both keys first keep the signed-in session and cart. The old-only server rejects the re-signed session. Django's native signer and ItsDangerous 2.1.2 also reject new-key values when the verifier lacks that key, and accept them when prepared."
            }
        }
    },
    "delivered": None,
    "recommendation": "suggestion",
    "band": None,
    "confidence": "medium",
    "rule_gap": "No earlier ruling settles incompatible key sets during a rolling deployment. Promised 5's platform paragraph is on trial; its missing-key analogue needs the owner's judgment. first-01 concerns the same rotation announcement but remains under an earlier reading. This recommendation does not settle that gap.",
    "strongest_argument_against": "The documented rotation recipe omits staging for an ordinary rolling deployment, and the broad announcement states no single-server restriction. If that promises this deployment, the verified session loss is not delivered and is a problem, other-material under boundary v4 exception 5 because base loses the same data.",
    "clauses": ["before-3", "promised-4a", "promised-5", "promised-6", "promised-8a"],
    "nearest": ["first-01", "second-09"]
}]
with (root / "refresh.json").open("x") as stream:
    json.dump(refresh, stream, indent=2)
    stream.write("\n")
