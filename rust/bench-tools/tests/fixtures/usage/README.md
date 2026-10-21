# Usage meter fixtures

Minimized copies of the Claude transcripts of two attempts in `bench/runs/2026-10-10-last-push-claude-ce-opus`, taken
from the attempts' `home/.claude/projects/<clone>/` directories. The originals are unchanged.

Each copy keeps only what `bench-tools usage claude` reads: the `type`, `timestamp` and `requestId` of each line, every
assistant line's `model`, `stop_reason` and `usage`, its tool calls' `id` and `name`, completion receipts (`status`,
`agentId`, `usage`, `content`), and `cost-state` session totals. Text is kept only in the subagent the receipt
repairs and in that receipt; every other text block reads `…`. Tool inputs are `{}`. The metered totals match the
full transcripts.

- `att-019`: subagent `a8161a8c3b5dc90d0` stops before its final request's last line. Its sole receipt repairs the
  output from 759 to 2,908 tokens, and the sums then match the session totals.
- `att-029`: subagent `a88f943c4abbb07bd` is repaired from 704 to 834 tokens, but three earlier requests in other
  subagents never finished, and 2,665 output tokens stay unexplained, so the attempt stays incomplete.
