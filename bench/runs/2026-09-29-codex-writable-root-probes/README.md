# Codex writable-root preflight

These are non-scored native `codex review` probes on the `toy-average` synthetic fixture. Each fresh attempt used the production layout `<attempt>/{clone,clone-cache,clone-work}`. The reviewer was instructed to write a marker to both sibling roots, read back and verify both values, remove both markers, then review `main...review-head`.

The captured native invocation set `sandbox_mode="workspace-write"`, `sandbox_workspace_write.network_access=false`, and `sandbox_workspace_write.writable_roots=["<attempt>/clone-cache", "<attempt>/clone-work"]`; it set the reviewer model and `model_reasoning_effort="high"`. The exact command and expanded paths are preserved in each `launcher.trace.txt`. Both audit records show the marker command, both attempt-local allowed roots, no guidance probes, and zero violations. The marker files were absent after each run. Both native outputs parsed and both attempt records are `valid completed`.

| Probe | Model | Cost | Archive |
| --- | --- | ---: | --- |
| `probes/luna-high` | GPT-6 Luna High | $0.001467 | [`att-001.tar.gz`](../../../artifacts/transcripts/2026-09-29-codex-writable-root-probes/att-001.tar.gz) |
| `probes/sol-high` | GPT-6 Sol High | $0.076451 | [`att-002.tar.gz`](../../../artifacts/transcripts/2026-09-29-codex-writable-root-probes/att-002.tar.gz) |

Each archive contains the unmodified raw rollout logs; `attempt.json` records its SHA-256 and successful restoration check. Full dispatch output, command trace, prompt, audit, normalized response, and usage rows are filed under each probe directory. The live attempt trees remain under `~/.t3/bench-runs/2026-09-29-codex-writable-root-probes/`.
