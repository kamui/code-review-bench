import re, sys, pathlib
skill = pathlib.Path(sys.argv[1]); run = pathlib.Path(sys.argv[2])
refs = skill / "references"
tmpl_lines = (refs / "subagent-template.md").read_text().splitlines()
assert tmpl_lines[8].strip() == "```" and tmpl_lines[191].strip() == "```", (tmpl_lines[8], tmpl_lines[191])
template = "\n".join(tmpl_lines[9:191])
intent = (
    "Avoid a redundant, slow `SSLContext.load_verify_locations()` on every verified HTTPS connection: "
    "build one module-level `SSLContext` preloaded with the default CA bundle "
    "(`extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)`) and pass it as `ssl_context` in the urllib3 pool kwargs when "
    "`verify=True`, while `cert_verify()` stops setting `ca_certs`/`ca_cert_dir` in that case. "
    "It must not regress what `verify=True`, `verify=False`, and a custom CA bundle path (file or directory, "
    "including zip-packaged certifi) mean for callers and adapter subclasses, and the shared context must be safe "
    "across threads, sessions, hosts, and client-certificate use."
)
pr = (run / "inputs/pr-context.md").read_text()
prior = (run / "inputs/prior-comments.md").read_text()
policy = (run / "inputs/execution-policy.md").read_text()
common = {
    "diff_scope_rules": (refs / "diff-scope.md").read_text(),
    "schema": (refs / "findings-schema.json").read_text(),
    "run_dir": str(run), "run_id": run.name, "intent_summary": intent,
    "file_list": (run / "files.txt").read_text().strip(),
    "diff": "BASE:8dd3b26bf59808de24fd654699f592abf6de581e (scope: local; working tree is the reviewed head)\n" + (run / "full.diff").read_text(),
}
for name in sys.argv[3:]:
    persona = (refs / "personas" / f"{name}-reviewer.md").read_text().replace("<root>", "docs")
    vals = dict(common, persona_file=persona, reviewer_name=name,
                pr_metadata=pr + ("\n\n" + prior if name == "previous-comments" else ""))
    seen = set()
    def sub(m):
        seen.add(m.group(1)); return vals[m.group(1)]
    out = re.sub(r"\{(" + "|".join(map(re.escape, vals)) + r")\}", sub, template)
    missing = set(vals) - seen
    assert not missing, missing
    out += "\n\n" + policy
    (run / "prompts" / f"{name}.prompt.md").write_text(out)
    print(name, len(out))
