# Artifact and checkout verification

The tracked checkout content was hashed before focused test execution and after report generation. Both passes used sorted `git ls-files -z` output, hashed each path and its file contents in order, and produced SHA-256 `ae4a82d88c6a6bdf08e6aa165bf20126d750f96e24f0c0d2921b9e4d9d954ae8`. `git status --porcelain=v1` was empty at both checks. The pinned HEAD remains `136a4850df9aec8cf8813b27ba1533d4a17bc642`. `git diff --check main...review-head` exited successfully.

The finding index contains two rows copied verbatim from the summary's actionable-finding paragraphs, with their exact headings and diff anchors. There are no question rows. Every indexed quote occurs in its reported file. The three subsystem details were written once and preserved unchanged while generating the index.

The offline internal tests passed: 24 passing and 15 skipped top-level tests, 5.586 seconds. Test output is preserved at `../review-test-output.txt`. Proposed code was neither applied nor compiled. Docker, live provider verification, build, vet, and lint were not run.

Precise supporting-source ranges: `enabledProviderAuth` starts at `internal/constants.go:120` (the detail reference includes its preceding blank line). `TestGiteaOrgsRepositoryBackup` ends at `internal/backup_test.go:637`; `assertGiteaOrgTwoOnlyBackedUp` spans lines 651–663; `assertGiteaAllOrgsBackedUp` spans lines 665–681. The test-detail prose gives approximate ranges for these helpers; use these exact ranges when navigating. The actionable finding anchors in the summary and index were checked directly and are accurate.

The final summary was wrapped as prose before regenerating the index. Every final index quote is a complete, verbatim finding paragraph including its line breaks. SHA-256 checks confirm all three subsystem detail files stayed unchanged during formatting and index generation.
