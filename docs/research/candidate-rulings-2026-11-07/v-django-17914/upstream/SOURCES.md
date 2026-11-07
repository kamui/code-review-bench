# Where these files come from

All fetched or produced on 2026-11-07 by this round. Files whose names start with `later-` hold records from after the cut-off (the last push, 2024-03-01T08:02:02Z) and are used only to confirm.

| File | Source |
| --- | --- |
| `pypi-psycopg-pool.json`, `pypi-psycopg.json` | `curl https://pypi.org/pypi/psycopg-pool/json`, `curl https://pypi.org/pypi/psycopg/json` (release upload times) |
| `owner-psycopg-pool-3.2.1--docs_advanced_pool.rst`, `owner-psycopg-pool-3.2.1--docs_api_pool.rst` | `gh api "repos/psycopg/psycopg/contents/docs/advanced/pool.rst?ref=pool-3.2.1"` and `.../docs/api/pool.rst?ref=pool-3.2.1` (raw); tag `pool-3.2.1` dated 2024-01-07T01:42:03Z (`gh api repos/psycopg/psycopg/git/tags/f3a690fb86d629e82f7bfaf9186201e914a5df2c`) |
| `owner-psycopg-pool-3.2.1-pool.py` | `psycopg_pool/pool.py` from the psycopg-pool 3.2.1 wheel installed with `uv pip install psycopg-pool==3.2.1` |
| `owner-docs-search.txt` | `rg` over the two psycopg pool documentation files; the command is its first line |
| `head-docs_ref_databases.txt`, `head-docs_ref_settings.txt`, `base-docs_ref_databases.txt` | `git show <sha>:docs/ref/...` from the commits fetched from https://github.com/django/django.git |
| `project-docs-search.txt` | `rg` over `docs/` at the head; the command is its first line |
| `change.diff`, `change-search.txt` | `git diff bcccea3e..fad334e1`; `rg` over it, the command is its first line |
| `maint-pr-17914.json`, `maint-pr-17594.json` | `gh api repos/django/django/pulls/<n>` |
| `maint-pr-<n>-review-comments.json`, `-issue-comments.json`, `-reviews.json` | `gh api --paginate repos/django/django/pulls/<n>/comments`, `.../issues/<n>/comments`, `.../pulls/<n>/reviews` (per_page=100) for 17914 and 17594 |
| `maint-pr-health-scan.txt` | a scan of the six files above; the pattern is in its header |
| `maint-search-q1.json` to `maint-search-q4.json`, `maint-search-queries.txt` | `gh api --method GET search/issues -f q=<query> -f per_page=100`; the four queries are in `maint-search-queries.txt` |
| `maint-trac-33497.html`, `maint-trac-34914.html`, `maint-trac-34839.html` | `curl https://code.djangoproject.com/ticket/<n>` |
| `later-trac-pool-summary.csv`, `later-trac-health-desc.csv` | `curl "https://code.djangoproject.com/query?summary=~pool&format=csv&col=id&col=summary&col=status&col=resolution&col=time&col=type&max=500"` and the same with `description=~CONN_HEALTH_CHECKS` |
| `later-trac-37283.html` | `curl https://code.djangoproject.com/ticket/37283` |
| `later-commit-516d06e48f.json` | `gh api repos/django/django/commits/516d06e48fb5a4e7deb055ef578fa1272e86d326` |
| `later-commits-databases-txt.json` | `gh api "repos/django/django/commits?path=docs/ref/databases.txt&since=2024-03-01T00:00:00Z&per_page=100"` |
| `later-main-docs_ref_databases.txt`, `later-main-docs_ref_settings.txt`, `later-main-postgresql-base.py`, `later-main-sha.txt` | `gh api "repos/django/django/contents/<path>?ref=main"` (raw) at main `a1f2b45ce5` of 2026-11-06 |
| `code-q1.json`, `code-q2.json` | `gh api --method GET search/code -f q='"pool": True "CONN_HEALTH_CHECKS" language:Python' -f per_page=10` and `-f q='"pool": True "django.db.backends.postgresql" language:Python'` |
| `code/<owner>_<repo>_<path>.txt`, `.dates.json` | `gh api repos/<owner>/<repo>/contents/<path>` (raw) and `gh api "repos/<owner>/<repo>/commits?path=<path>&per_page=100"` (first and last commit dates) for five hits of `code-q1.json` |
| `later-5.1-postgresql-base.py`, `later-tag-5.1.txt` | `gh api "repos/django/django/contents/django/db/backends/postgresql/base.py?ref=5.1"` (raw); `gh api repos/django/django/git/refs/tags/5.1 --jq .object.sha` (tag object; tagger date 2024-08-07T13:28:18Z) |
| `code/onnela-lab_beiwe-backend_config_django_settings.py.commits.json` | `gh api "repos/onnela-lab/beiwe-backend/commits?path=config/django_settings.py&per_page=100"` |

SHA-256 of each file:

- `base-docs_ref_databases.txt` `ce6d19e9d1060d1d72010f31b325fcbbc287cfa7f73335523deaf0496d044c46`
- `change-search.txt` `95d3bc2a56efead010ff1e713cc851ffe4eb52a225f483c540da9b493582a038`
- `change.diff` `78881455d7c13668f7d2ef975a4913000f9bef74bd777ad41e34b293ef76d151`
- `code-q1.json` `26d45d3e44b9c01b2e0d530185f0323bf29228d36df70f3b5741a0fd4b07c7fe`
- `code-q2.json` `1800438a37bcb845047d7892ef8fe97bfc960f8c0cba1ccbe8f61dac39497568`
- `code/buildingSMART_validate_backend_core_settings.py.dates.json` `56744fc1787c376c47f75be0b51d6eb54c0f2374f1b940b94692a71545f56aee`
- `code/buildingSMART_validate_backend_core_settings.py.txt` `a9e24535c10db94ac3d6ec6e5a9edc958cd1d17301078d10133dcef4aa1b628f`
- `code/dimagi-rad_scout_apps_common_db_pool.py.dates.json` `f20374f60a11dec9f12c6a8f16a61c1c61dfa792341995ca0ed5a30d0a3a465b`
- `code/dimagi-rad_scout_apps_common_db_pool.py.txt` `c09427144242d1ac428ccd4d0dee5ad64c855e92fb367b54fa48700556dbb036`
- `code/nineaiyu_xadmin-server_server_settings_databases.py.dates.json` `12e0d39e4a5ac874056e21dae6d4d4818ee4611516b730d175eda1c3c55788bf`
- `code/nineaiyu_xadmin-server_server_settings_databases.py.txt` `7f90f11dc5a1db0ed82d5faeff01c646342e7f68a37f90f555b0b365b3ed1e08`
- `code/onnela-lab_beiwe-backend_config_django_settings.py.commits.json` `4144b9871bb6f9922755c863c63dd6e1a478cd5bd8bdf6c8da2cec60829341e7`
- `code/onnela-lab_beiwe-backend_config_django_settings.py.dates.json` `4f165b8923bfc9da576968b8064b943bfc71ae890992ea379c2f8d08a6fe7857`
- `code/onnela-lab_beiwe-backend_config_django_settings.py.txt` `08849c9314ecb667cf18170b04a6841aad1948a500cb3ebca2fb6c1da557522d`
- `code/zentralopensource_zentral_server_server_settings.py.dates.json` `a78d8711a6aa756db7e0b0483ba51033e951fde003ebe8d585b8ca507cc03033`
- `code/zentralopensource_zentral_server_server_settings.py.txt` `1f302e91ed35a170e1580a8c79c218bb812c6efc09f7850bb41392f9ce6bd2b1`
- `head-docs_ref_databases.txt` `325d60bd97e2787895ad028b600cb3c850cbc93d9fcbe694cf2a1a043e8b95b5`
- `head-docs_ref_settings.txt` `a5cabcffcd4af2548451f35ce29513f644be3f5a8dab9d1aacc6aa30db43e049`
- `later-5.1-postgresql-base.py` `7ce477bc59673b1fcfd4a152d3e1dcf648c86407824f78bd99186c771e063d81`
- `later-commit-516d06e48f.json` `5c95f3c8c0cfbbb16f5cc0442e2e59adb47c6c5092074f6b79cadb1ec401726f`
- `later-commits-databases-txt.json` `81307b861409cfb0dc905f371d13716ffb62ff791f7a5c5f3065848ace3e257a`
- `later-main-docs_ref_databases.txt` `4d319dcf690796f9bca4fb8496cc1a93ee5d0186bdcf5294d496e5af6fc419a7`
- `later-main-docs_ref_settings.txt` `9a654a29c87bb88a7548eceebeac3ae39a7f9a3ea565ae504b3454207c99630f`
- `later-main-postgresql-base.py` `2ceb0e72789f328fb5dbdf8ecd7a617876d1594e83977328c9936cd662152f1c`
- `later-main-sha.txt` `06d529b493664dd897f5c06871c107ebf36def0389f9c3f9f6a7732ba8638947`
- `later-tag-5.1.txt` `abf3690d5e7a3e762424586cdd0fde6f763e97960de08b120084caebe9542045`
- `later-trac-37283.html` `45cd4cf71eb18eb116cfd8882b3b6264d92b79b433e65aeec90b19d7d696bd8d`
- `later-trac-health-desc.csv` `abac5c13923c56f08502359948ec3d238fa578f63e3d873a42db28cbb50c7f73`
- `later-trac-pool-summary.csv` `73a81bdbee737139b7dd6714c1984c6a05678df55c81abb25f3e8036f4348ce9`
- `maint-pr-17594-issue-comments.json` `267d156bb96f439d481ec4677ade375651c029f34db3ee967f3fc7df64e4da91`
- `maint-pr-17594-review-comments.json` `d80a783f810461bbceb16c16f6b796d901909d3cae9668a5e134d8b877111b8c`
- `maint-pr-17594-reviews.json` `03fb7b63cc72bcd5e0839e6e8479660d23dad088fa4fc156ef043258dd5a0fe5`
- `maint-pr-17594.json` `66c4c2ab8546270531afdf25c532dff544f12c543bdf1b50756b6ea6101c11fe`
- `maint-pr-17914-issue-comments.json` `27cb6af09199d8cfb94cc83e2660397df7f45a02cbd9703ef4a87b539e744160`
- `maint-pr-17914-review-comments.json` `82279e809800e5eda031944adcdb836d2576604152400f154a73b4e26a96a4c8`
- `maint-pr-17914-reviews.json` `0056a479a914555404cd48954e437102ded3536b077a54b167a8e7ec19328f78`
- `maint-pr-17914.json` `711eceb0cab56e1d53808734a0ab5e9cc095be3f2b52e445dfd4ec62fd0f01ea`
- `maint-pr-health-scan.txt` `69811b54f03a2f64b89f24db75e7d58bc7181d63e2b612f5c0e1dbfef8a08f5a`
- `maint-search-q1.json` `42083564079f720d5f863e5f373effe4d45b69c7fd9d0078c7d4a03dab0b67a8`
- `maint-search-q2.json` `3adebd6fecc49a7e2918146e93ab58631e0bf137f9ada4519ba93f899779475a`
- `maint-search-q3.json` `c9938edecb99d754b2d039ac9eec320a94c769b6a6417921e2dcbef9e2fe01a0`
- `maint-search-q4.json` `c9938edecb99d754b2d039ac9eec320a94c769b6a6417921e2dcbef9e2fe01a0`
- `maint-search-queries.txt` `38242ef01e1e1e51f52ae819c91a9f38ed98b31d1f9ede0c353772e4999cf07e`
- `maint-trac-33497.html` `943dfce89fff53b3adbbddc75af62cad1142709a4ac665dd90c30155e8966b4f`
- `maint-trac-34839.html` `6531c979b2f6809418980affbf0aa611c7dd657ac703c9289a5d1bc0d287f8e8`
- `maint-trac-34914.html` `91b3cd1b493650eae9f95a42c42863c007f43a3af488f046e9b4dca73b745055`
- `owner-docs-search.txt` `3572ddb09bd9739e1385fd6901f89f96a9d1295f639df9a63360a492b35429e3`
- `owner-psycopg-pool-3.2.1--docs_advanced_pool.rst` `df9e39797585cc83e7d7d81ab4d4ac395ce4ba14f4dfd11c19bde7afce04ab39`
- `owner-psycopg-pool-3.2.1--docs_api_pool.rst` `702f7565238af4c56dc0032a9fff634437af2051d6372ef4b93d5e40f0804812`
- `owner-psycopg-pool-3.2.1-pool.py` `6f642e7b1bd41ed29e94817251bc48f51ebe3ec0d46661eeeed046374763850e`
- `project-docs-search.txt` `279a8f68240e7c6ab19b3c38f396ab6736b1c35dba3dfe8d2275449bed1a8f94`
- `pypi-psycopg-pool.json` `48d34a17066ce38d0b81e511cbdda0d1d7a45e2f2d5ea1c8614451610d6026ed`
- `pypi-psycopg.json` `2a49f5497fd34ad85d7b312fb91792e62a97f972d1fd78ff75d76265a188cc9f`
