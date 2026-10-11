# Final execution and checkout verification

The clone was clean before review and remains clean after report generation. The following read-only commands exited successfully. Empty output is explicitly identified.

```text
$ git rev-parse HEAD main
14d39e5d1ad6b7aca2fb067415dba09c6bea219b
30b8ea2004fa999bed151204208676c6c0a9d261
```

```text
$ git status --porcelain=v1
(no output)
```

```text
$ git diff --exit-code
(no output)
```

```text
$ git diff --cached --exit-code
(no output)
```

```text
$ git ls-files -s packages/react/src/field/control/FieldControl.tsx packages/react/src/field/control/FieldControl.test.tsx
100644 a548b2ac1d920d93346cc285ff966ae52c5a65a0 0	packages/react/src/field/control/FieldControl.test.tsx
100644 06941f827bb4960737cb6f21c3b0a023ffda0ea8 0	packages/react/src/field/control/FieldControl.tsx
```

The unchanged changed-file index blobs are also verified against their working-tree content using `git hash-object`, without writing objects.

`packages/react/src/field/control/FieldControl.tsx`: `06941f827bb4960737cb6f21c3b0a023ffda0ea8`.

`packages/react/src/field/control/FieldControl.test.tsx`: `a548b2ac1d920d93346cc285ff966ae52c5a65a0`.

Reports and scratch files are outside the clone in clone-work. Existing dependencies were used offline. No delegation, network requests, browsers, checkout edits, or applied remedies were used. Each test command completed in seconds.
