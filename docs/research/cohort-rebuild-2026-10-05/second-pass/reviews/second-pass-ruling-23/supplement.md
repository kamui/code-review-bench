# Second pass, ruling 23: which version of psycopg-pool a person would have had

Fetched on 2026-10-06 after the user asked: "At the time this requirement was added for connection pooling, was the prevalent version or the version that gets installed by default greater than 3.2.0?"

The cut-off for Django PR 17914 is 2024-03-02.

## Read

- Release dates, from PyPI (`pypi-psycopg-pool.json`, fetched 2026-10-06):

  | Version | Released | Needs Python |
  | --- | --- | --- |
  | 3.1.8 | 2023-09-23 | 3.7 or later |
  | 3.2.0 | 2023-11-11 | 3.8 or later |
  | 3.1.9 | 2023-11-11 | 3.7 or later |
  | 3.2.1 | 2024-01-07 | 3.8 or later |
  | 3.2.2 | 2024-05-10 | 3.8 or later |

- 3.1.9 came out on the same day as 3.2.0. It is a maintenance release of the older line, for Python 3.7.
- At the cut-off the newest release was 3.2.1.
- Django at the pinned head needs Python 3.10 or later (`setup.cfg`, `python_requires = >=3.10`). Every Python that can run it can install 3.2.x.
- psycopg 3.1.18 declares its pool extra as `psycopg-pool` with no minimum (`pypi-psycopg-3.1.18.json`).
- Django's own test requirements at the pinned head pin `psycopg-pool>=3.2.0` (`tests/requirements/postgres.txt`). The commit before the change has no such line.
- Django's documentation at the pinned head states a minimum for the driver: "`psycopg` 3.1.8+ or `psycopg2` 2.8.4+ is required" (`docs/ref/databases.txt`, line 118). It states none for the pool package.

## What follows

- **The version installed by default.** A fresh `pip install "psycopg[pool]"` or `pip install psycopg-pool` on any Python that runs this Django would have selected 3.2.1 at the cut-off, and 3.2.0 from 2023-11-11. Both work.
- **Who would have an older one.** Someone who installed the pool package before 2023-11-11 and did not upgrade it, or whose lock file or constraint holds it below 3.2. At the cut-off the working versions had been the newest for under four months.

## Not established

- **Which version was prevalent.** The share of installations on 3.1.x against 3.2.x at the cut-off was not found. PyPI's public download statistics do not reach back to 2024.
- Whether any Django user met the failure. No report was found (the dossier's searches).
