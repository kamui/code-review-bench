### P1-oauth-complete

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | none | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-p1 | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq5 | none | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| both-peq5+p1 | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq1-envtrue | none | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq1-lfslines | none | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq2 | none | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq3 | none | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq4 | none | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-wide | none | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |

### P1-email-only-with-github

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | backups to keep: 3, compare method: refs, backup LFS: true | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-p1 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq5 | backups to keep: 3, compare method: refs, backup LFS: true | no | provider configuration invalid | 1 | GitHub compare method: clone |
| both-peq5+p1 | none | no | provider configuration invalid | 1 | GitHub compare method: clone |
| fix-peq1-envtrue | backups to keep: 3, compare method: refs, backup LFS: true | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq1-lfslines | backups to keep: 3, compare method: refs, backup LFS: true | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq2 | backups to keep: 3, compare method: refs, backup LFS: true | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq3 | backups to keep: 3, compare method: refs, backup LFS: true | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq4 | backups to keep: 3, compare method: refs, backup LFS: true | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-wide | backups to keep: 3, compare method: refs, backup LFS: true | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |

### P1-email-only-alone

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | backups to keep: 3, compare method: refs, backup LFS: true | no | provider configuration invalid | 1 |  |
| fix-p1 | none | no | provider configuration invalid | 1 |  |
| fix-peq5 | backups to keep: 3, compare method: refs, backup LFS: true | no | provider configuration invalid | 1 |  |
| both-peq5+p1 | none | no | provider configuration invalid | 1 |  |
| fix-peq1-envtrue | backups to keep: 3, compare method: refs, backup LFS: true | no | provider configuration invalid | 1 |  |
| fix-peq1-lfslines | backups to keep: 3, compare method: refs, backup LFS: true | no | provider configuration invalid | 1 |  |
| fix-peq2 | backups to keep: 3, compare method: refs, backup LFS: true | no | provider configuration invalid | 1 |  |
| fix-peq3 | backups to keep: 3, compare method: refs, backup LFS: true | no | provider configuration invalid | 1 |  |
| fix-peq4 | backups to keep: 3, compare method: refs, backup LFS: true | no | provider configuration invalid | 1 |  |
| fix-wide | backups to keep: 3, compare method: refs, backup LFS: true | no | provider configuration invalid | 1 |  |

### P1-apitoken-complete

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-p1 | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq5 | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| both-peq5+p1 | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq1-envtrue | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq1-lfslines | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq2 | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq3 | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-peq4 | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |
| fix-wide | backups to keep: 3, compare method: refs, backup LFS: true | yes | backups complete | 0 | git-lfs called (1); using diff remote method: refs |

### PEq5-token-misnamed-with-github

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | compare method: clone | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-p1 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq5 | compare method: clone | no | provider configuration invalid | 1 | GitHub compare method: clone |
| both-peq5+p1 | none | no | provider configuration invalid | 1 | GitHub compare method: clone |
| fix-peq1-envtrue | compare method: clone | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq1-lfslines | compare method: clone | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq2 | compare method: clone | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq3 | compare method: clone | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq4 | compare method: clone | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-wide | compare method: clone | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |

### PEq5-oauth-partial-with-github

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-p1 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq5 | none | no | provider configuration invalid | 1 | GitHub compare method: clone |
| both-peq5+p1 | none | no | provider configuration invalid | 1 | GitHub compare method: clone |
| fix-peq1-envtrue | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq1-lfslines | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq2 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq3 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq4 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-wide | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |

### PEq1-file

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-p1 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq5 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| both-peq5+p1 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq1-envtrue | none | no | backups complete | 0 | GitHub LFS line; git-lfs called (1); GitHub compare method: clone; using default diff remote method: clone |
| fix-peq1-lfslines | none | no | backups complete | 0 | GitHub LFS line; git-lfs called (1); GitHub compare method: clone; using default diff remote method: clone |
| fix-peq2 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq3 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq4 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-wide | none | no | backups complete | 0 | GitHub LFS line; git-lfs called (1); GitHub compare method: clone; using default diff remote method: clone |

### PEq2-file-only

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | none | no | environment variable GIT_BACKUP_DIR must be set | 1 | GitHub compare method: clone |
| fix-p1 | none | no | environment variable GIT_BACKUP_DIR must be set | 1 | GitHub compare method: clone |
| fix-peq5 | none | no | environment variable GIT_BACKUP_DIR must be set | 1 | GitHub compare method: clone |
| both-peq5+p1 | none | no | environment variable GIT_BACKUP_DIR must be set | 1 | GitHub compare method: clone |
| fix-peq1-envtrue | none | no | environment variable GIT_BACKUP_DIR must be set | 1 | GitHub compare method: clone |
| fix-peq1-lfslines | none | no | environment variable GIT_BACKUP_DIR must be set | 1 | GitHub compare method: clone |
| fix-peq2 | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |
| fix-peq3 | none | no | environment variable GIT_BACKUP_DIR must be set | 1 | GitHub compare method: clone |
| fix-peq4 | none | no | environment variable GIT_BACKUP_DIR must be set | 1 | GitHub compare method: clone |
| fix-wide | none | no | backups complete | 0 | GitHub compare method: clone; using default diff remote method: clone |

### PEq3-compare-file

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | none | no | backups complete | 0 | GitHub compare method: refs; using default diff remote method: clone |
| fix-p1 | none | no | backups complete | 0 | GitHub compare method: refs; using default diff remote method: clone |
| fix-peq5 | none | no | backups complete | 0 | GitHub compare method: refs; using default diff remote method: clone |
| both-peq5+p1 | none | no | backups complete | 0 | GitHub compare method: refs; using default diff remote method: clone |
| fix-peq1-envtrue | none | no | backups complete | 0 | GitHub compare method: refs; using default diff remote method: clone |
| fix-peq1-lfslines | none | no | backups complete | 0 | GitHub compare method: refs; using default diff remote method: clone |
| fix-peq2 | none | no | backups complete | 0 | GitHub compare method: refs; using default diff remote method: clone |
| fix-peq3 | none | no | backups complete | 0 | GitHub compare method: refs; using diff remote method: refs |
| fix-peq4 | none | no | backups complete | 0 | GitHub compare method: refs; using default diff remote method: clone |
| fix-wide | none | no | backups complete | 0 | GitHub compare method: refs; using diff remote method: refs |

### PEq4-gitea-url-only

| Variant | BitBucket lines at start-up | BitBucket backup ran | End of run | Exit | Other |
| --- | --- | --- | --- | --- | --- |
| head | none | no | all backups failed | 0 |  |
| fix-p1 | none | no | all backups failed | 0 |  |
| fix-peq5 | none | no | all backups failed | 0 |  |
| both-peq5+p1 | none | no | all backups failed | 0 |  |
| fix-peq1-envtrue | none | no | all backups failed | 0 |  |
| fix-peq1-lfslines | none | no | all backups failed | 0 |  |
| fix-peq2 | none | no | all backups failed | 0 |  |
| fix-peq3 | none | no | all backups failed | 0 |  |
| fix-peq4 | none | no | provider configuration invalid | 1 |  |
| fix-wide | none | no | all backups failed | 0 |  |

