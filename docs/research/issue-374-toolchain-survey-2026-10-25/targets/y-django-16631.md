# y-django-16631 (django/django PR 16631, "SECRET_KEY_FALLBACKS in get_user()")

Desk survey only. Nothing was built, installed or run. Labels: **read** = read in a file (named), **reported** = target.json / receipt / grader text, **inferred** = my reasoning.

## 1. Toolchain at review-head

| Fact | Value | Label |
| --- | --- | --- |
| Python | `python_requires = >=3.10` (setup.cfg); classifiers 3.10, 3.11 | read: setup.cfg |
| Runtime deps | `asgiref >= 3.6.0`, `sqlparse >= 0.2.2` (`tzdata` win32 only) | read: setup.cfg |
| Provisioned venv | CPython 3.10.12 (`/usr/bin/python3.10`), asgiref 3.12.1, sqlparse 0.6.0, typing_extensions 4.16.0; 116 KB archive; `post_clone` empty | reported: cache receipt; read: `tar -tzf` listing of the archive |
| Sphinx / docutils in venv | absent (no `sphinx*`, `docutils*`, `jinja2*`, `pygments*` entries in the archive) | read: archive listing |
| Test runner | `tests/runtests.py <label> --settings=test_sqlite` (sqlite in-memory, `tests/test_sqlite.py` ships with the repo) | reported: target.json allowance and smoke |
| Docs requirements | `docs/requirements.txt`: `pyenchant`, `Sphinx>=4.5.0`, `sphinxcontrib-spelling`, `blacken-docs`. Sphinx is unpinned | read: docs/requirements.txt |
| Docs config | `docs/conf.py`: `needs_sphinx = "4.5.0"`; extensions `djangodocs` (local, `docs/_ext/djangodocs.py`), `sphinx.ext.extlinks`, `sphinx.ext.intersphinx`, `sphinx.ext.viewcode`, `sphinx.ext.autosectionlabel`; `html_theme = "djangodocs"` with `html_theme_path = ["_theme"]` (theme is in the repo); `root_doc = "contents"`; `default_role = "default-role-error"` | read: docs/conf.py |
| Spelling extension | `sphinxcontrib.spelling` (needs `pyenchant` and the libenchant C library) is appended only when the literal string `spelling` is in `sys.argv`. A plain HTML/dummy build does not need it | read: docs/conf.py lines ~76-78 |
| `docs/_ext/djangodocs.py` imports | `docutils` (nodes, Directive, ViewList), `sphinx` (addnodes, version_info, `sphinx.builders.html.StandaloneHTMLBuilder`, `sphinx.directives.code.CodeBlock`, `sphinx.domains.std.Cmdoption`, `sphinx.errors`, `sphinx.util.logging`, `sphinx.util.console.bold`, `sphinx.writers.html.HTMLTranslator`). It uses `self._table_row_indices` (Sphinx >= 4.3) | read: docs/_ext/djangodocs.py |
| Network use by docs build | `intersphinx_mapping` points at docs.python.org, sphinx-doc.org, psycopg.org (fetches `objects.inv` at builder-init). `extlinks` only builds URLs | read: docs/conf.py |
| Docs size | about 600 `.txt` sources (about 450 are release notes) | read: `ls-tree` of docs |

## 2. Commands the graded claims needed

The change touches `django/contrib/auth/__init__.py`, `base_user.py`, `docs/ref/contrib/auth.txt`, `docs/topics/auth/customizing.txt`, `docs/releases/4.1.8.txt`, `tests/auth_tests/test_basic.py`.

The docs side of the diff (read, `git diff main review-head`), `docs/ref/contrib/auth.txt` (get_user):

```
:meth:`~django.contrib.auth.models.AbstractBaseUser.\
get_session_auth_fallback_hash`.
```

The role target contains a backslash then a newline (continuation line inside an inline role). The method is declared in `docs/topics/auth/customizing.txt` as `.. method:: models.AbstractBaseUser.get_session_auth_fallback_hash()` (under a `currentmodule` of `django.contrib.auth`).

| # | Check | Argv a grader would want (from clone root) | Claims (first / second) | Example id |
| --- | --- | --- | --- | --- |
| A | Does the escaped-newline `:meth:` role render/resolve, or break the docs build ("Sphinx build") | `<cache>/venv/bin/python -m sphinx -b dummy -d <work>/doctrees -D language=en docs <work>/out` (or `-b html`) with `PYTHONPATH=<clone>` | 3 first (all refuted by reasoning, "Sphinx build unavailable under the run allowance"), 2 second (both left `unresolved`: "provisioned environment lacks documentation tooling") = 5 claims | first `blind-3e2e2b/8/c10`; second `blind-a30271/8/c10`, `blind-950e81/8/c8` |
| B | (not a toolchain gap) claims 9885c7/1/c1 and adec35/1/c1: `get_user()` calls `get_session_auth_fallback_hash()` on a duck-typed user that does not subclass `AbstractBaseUser` | n/a | 2 second | `blind-9885c7/1/c1` |

Row B: the matching grader text only says the linked attachment (`01_session_auth_fallbacks.md`, `01_auth_session_hash.md`) "was not available through the supplied grading inputs". That is a packet problem, not a toolchain one. The behaviour (AttributeError on a non-`AbstractBaseUser` user) is checkable today with `tests/runtests.py auth_tests.test_basic --settings=test_sqlite`, which the profile already allows. No other refusals exist for this target; no `runtests.py` run was refused.

So the only new toolchain need is Sphinx (row A). Everything the claims need is a single docs parse, and only two source files matter (`docs/ref/contrib/auth.txt`, `docs/topics/auth/customizing.txt`).

## 3. Can it run offline today, and what provisioning would make it run

Today: no. Sphinx, docutils, Jinja2, Pygments etc. are not in the venv or archive (read), `post_clone` is empty (reported), and the profile allows only `tests/runtests.py <one label> --settings=test_sqlite` (reported). Even a docs build would be refused. Both second-pass graders say exactly that (reported).

What provisioning would be needed (online, once, into `{cache}/venv`):

- Install **Sphinx only**, not `docs/requirements.txt`. `sphinxcontrib-spelling` and `pyenchant` are not needed (spelling extension is argv-gated, inferred from conf.py), and `blacken-docs` is not a docs-build dependency (it pulls in `black`). `pyenchant` additionally needs the libenchant system library, which would be a host install.
- Version choice. `docs/requirements.txt` says `Sphinx>=4.5.0` (read), so the repo does not pin one. On the venv's Python 3.10 an unpinned install resolves to Sphinx 8.1.x (inferred: 8.2+ requires Python 3.11; I did not query PyPI). `djangodocs.py` imports `sphinx.util.console.bold`, subclasses `StandaloneHTMLBuilder` and `HTMLTranslator` and touches `_table_row_indices`, which are the sort of things that drift between Sphinx majors (inferred, not tested). The era-correct choice for review-time (2023-03-08) is Sphinx 6.1.3 (from memory, released 2023-01), which needs `docutils>=0.18,<0.20`, `Jinja2>=3`, `Pygments>=2.12`, `babel>=2.9`, `alabaster<0.8`, `imagesize`, `requests`, `packaging`, `snowballstemmer`, `sphinxcontrib-applehelp/devhelp/htmlhelp/jsmath/qthelp/serializinghtml`, `MarkupSafe`, `certifi`, `charset-normalizer`, `idna`, `urllib3` (all pure Python or wheels; inferred from memory of the metadata). Suggested build line mirroring the existing recipe: `uv pip install --quiet --python {cache}/venv/bin/python "Sphinx==6.1.3" "docutils<0.20"`. This is a design decision; pin and then run the build once in the cache-build smoke to prove the `djangodocs` extension loads (inferred risk).
- Archive size grows from about 116 KB to roughly 10 to 15 MB (inferred).
- The venv's `bin/python` is a symlink to `/usr/bin/python3.10` (the target.json says the interpreter lives outside the archive), so no extra mount is needed: `/usr` is already mounted.
- No other mount: no node/bun needed; the theme is in the repo.

Run-time behaviour (all inferred, since nothing was run):

- **Network:** the `intersphinx` extension tries to download `objects.inv` from three hosts when the builder starts. With no network the fetches fail fast and Sphinx emits "failed to reach any of the inventories" warnings, which are non-fatal unless `-W` is given. Unresolved Python-domain references to external objects are only reported with `-n` (nitpicky; the Makefile uses `-n`). A grader looking for warnings about the `get_session_auth_fallback_hash` reference should filter. If a clean log is wanted, provision could pre-seed no inventories (Sphinx's `intersphinx_mapping` cannot be overridden usefully with `-D`; the mapping is a dict). Dropping extensions with `-D extensions=...` is possible but then `-D` becomes an arbitrary-extension vector (see section 5).
- **Writes:** the clone is read-only, so `-d` (doctrees) and the output directory must be under `clone-work/`. Set `PYTHONDONTWRITEBYTECODE=1` or a `PYTHONPYCACHEPREFIX` so Python does not try to write `docs/_ext/__pycache__` (a failed write is silent, so this is cosmetic).
- **Time:** Sphinx reads the whole doc tree even when only some targets are requested (environment build), about 600 files. `djangodocs` returns `parallel_read_safe: True`, so `-j auto` can be used. I expect 1 to 3 minutes for a read-only `dummy` build on this host (inferred; Django's own docs build is known to be a few minutes for HTML with all pages). Use `-b dummy` (reads and resolves cross references, writes nothing), not `html`/`djangohtml` (writes thousands of files, runs `finish()` which also writes `templatebuiltins.js`), to stay under the five-minute limit. `-b dummy` does emit unresolved-reference warnings (inferred: reference resolution happens in the build's write phase, before the dummy builder's no-op `write_doc`).
- Cheaper check for exactly this claim, which still needs the same install: a docutils-only parse of the two lines would show whether the escape-newline is removed. Docutils comes in with Sphinx, so no extra provisioning. But the current profile forbids `-c`/script paths for this target (only `runtests.py`), so the profile shape would need to widen anyway.

Profile shape if added (the profile for this target is currently `runtests.py`-only):

- `<cache>/venv/bin/python -m sphinx -b dummy [-q|-n] [-j N] -d <clone-work>/doctrees -D language=en docs <clone-work>/out`, with `PYTHONPATH=<clone>`; one fixed argv template with only the two writable paths free.

## 4. Service-only checks

None. Docs build, auth tests and the duck-typing probes need no database server, Redis, browser or network (intersphinx failing offline is a warning, not a failure).

## 5. Risks for a focused profile

- `python -m sphinx` loads `conf.py` (Python executed). The clone's `docs/conf.py` is read-only and trusted, but these options let a grader run other code: `-c <confdir>` (a conf.py in `clone-work`), `-D extensions=...`, `-D`/`-A` arbitrary overrides, `-C` (no conf), `-b` custom builders, `-M` make-mode, `-w`/`-W` file writes (write path is under work, but the profile should fix it). Fix the argv template rather than allowing free flags: allow only `-b dummy|text`, `-n`, `-q`, `-j`, `-d <under work>`, `-D language=en`, and require that source dir is exactly `docs`.
- Extensions are importable from the clone (`docs/_ext`) and from the venv; graders cannot write into the clone, so the only uncontrolled Python is the repository's own.
- Intersphinx makes outbound connection attempts in the sandbox (loopback only, so failures); no data leaves. Expect noisy warnings. A `-D intersphinx_timeout=1` would shorten waits if DNS hangs instead of failing (inferred).
- Time: one docs build per command; do not allow `-b html` or `-E`/`-a` repeats; five-minute limit may be tight on a slow disk (inferred).
- Sphinx version drift (section 3) is the main correctness risk: an unpinned install could produce unrelated errors in `djangodocs`, and graders would then wrongly report "docs build fails".
- Non-docs risks: none new. `runtests.py` runs the repository's tests, which is already accepted for this target.
