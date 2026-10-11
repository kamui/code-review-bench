import re, sys, pathlib
skill = pathlib.Path(sys.argv[1]); run_dir = pathlib.Path(sys.argv[2]); run_id = run_dir.name
ref = skill / "references"
tpl_src = (ref / "subagent-template.md").read_text()
start = tpl_src.index("```\nYou are a specialist code reviewer.") + 4
end = tpl_src.index("</review-context>\n```") + len("</review-context>\n")
tpl = tpl_src[start:end]
common = {
    "diff_scope_rules": (ref / "diff-scope.md").read_text(),
    "schema": (ref / "findings-schema.json").read_text(),
    "pr_metadata": (run_dir / "pr-context.md").read_text(),
    "run_id": run_id, "run_dir": str(run_dir),
    "intent_summary": (run_dir / "intent.txt").read_text().strip(),
    "file_list": (run_dir / "files.txt").read_text().strip(),
    "diff": (run_dir / "full.diff").read_text(),
}
env = (run_dir / "environment.md").read_text()
personas = {"correctness": "correctness-reviewer", "testing": "testing-reviewer", "security": "security-reviewer",
            "api-contract": "api-contract-reviewer", "performance": "performance-reviewer",
            "reliability": "reliability-reviewer", "adversarial": "adversarial-reviewer"}
for name, asset in personas.items():
    vals = dict(common, reviewer_name=name, persona_file=(ref / "personas" / f"{asset}.md").read_text())
    missing = set()
    def sub(m):
        k = m.group(1)
        if k in vals: return vals[k]
        missing.add(k); return m.group(0)
    out = re.sub(r"\{([a-z_]+)\}", sub, tpl)
    out = out.replace("<root>", "docs") + env
    (run_dir / "prompts" / f"{name}.md").write_text(out)
    print(name, len(out), "unresolved:", sorted(missing))
