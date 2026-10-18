# Domain docs

This repo uses a single-context layout:

- `GLOSSARY.md` at the repo root defines domain terms.
- `docs/adr/` records architectural decisions.

## Before exploring the codebase

Read `GLOSSARY.md` and ADRs relevant to the area you are about to work in.

If a domain document is absent, proceed silently. The `/domain-modeling` skill creates domain documents when terms or decisions are resolved.

## Use the glossary's vocabulary

Use the terms defined in `GLOSSARY.md` when naming domain concepts in issues, proposals, hypotheses, tests, and other output. Follow each term's stated alternatives to avoid.

If a concept is missing, check whether the project already has a name for it. Note a real vocabulary gap for `/domain-modeling`.

## Flag ADR conflicts

If a proposal contradicts an existing ADR, name the ADR and explain why the decision needs to be reopened.
