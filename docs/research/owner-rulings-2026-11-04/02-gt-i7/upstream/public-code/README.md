# Public code read for the search in public-code-search.json

Fetched 2026-11-04 with `gh api repos/<repo>/contents/<path>?ref=<sha>`, each at the commit the search returned. Commit dates from `gh api repos/<repo>/commits/<sha>`. All are after the 2024-05-15 cut-off; they show current practice only.

| File | Repository and path | Commit | Committed | What its cert_verify does |
| --- | --- | --- | --- | --- |
| `geopy_geopy.py` | geopy/geopy `geopy/adapters.py` | `402cbba8` | 2026-07-12 | clears conn.ca_certs, ca_cert_dir, cert_file and key_file when the adapter has its own context |
| `SFDO-Tooling_CumulusCI.py` | SFDO-Tooling/CumulusCI `cumulusci/utils/http/requests_utils.py` | `95952755` | 2026-10-30 | clears conn.ca_certs when it is the default bundle |
| `mapillary_mapillary_tools.py` | mapillary/mapillary_tools `mapillary_tools/http.py` | `b52b21e5` | 2026-11-04 | clears conn.ca_certs to fall back on the OS store |
| `cycodehq_cycode-cli.py` | cycodehq/cycode-cli `cycode/cyclient/cycode_client_base.py` | `f5580bf7` | 2026-11-04 | clears conn.ca_certs |
| `Federated-Industrial-Laboratories_ficc.py` | Federated-Industrial-Laboratories/ficc `src/ficc/windows_http.py` | `b87adb85` | 2026-10-28 | sets CERT_REQUIRED and clears conn.ca_certs and ca_cert_dir, without calling the parent |
