The assessment used only rule.md, earlier-rulings.md, and the four case files as inputs.
All admission reasons use read/run facts available at the cut-off; later fixes and later statements supply no premise.
Calling expiry cleanup intended does not establish that losing concurrent writes or their names was intended; exact faulty intent is not shown.
The Redis warning that directory loss may happen acknowledges a limitation, but does not explicitly declare these operations unsupported under promised-1.
For P2, backend non-atomicity does not withdraw the separate written promise of serialization across filers; no protocol restriction is stated.
Before-4: C2na/PE-s3 share line 216, one tested fix, and deletion based on a stale expiry decision; C2nb/PE-s2 share lines 216-217, one tested fix, and removal of a recreated record's name.
P1 and P2 have distinct causal lines and fixes from the named problems; P2's similar outcome alone does not join it to PE-s2.
P2's unforced head order writes after ZREM and needs the UpdateEntry fix; it does not establish that the P2 patch cures every overlapping-write order or joins the entire case to GT-s2.
Reversing facts, settled by the runs: preservation of the replacement in C2na, preservation of its directory name in C2nb/P2, or honoring the documented setting in P1 would remove the respective non-delivery.
Accidental filenames in Python launcher diagnostics: /home/jack/.config/mise/config.toml, /home/jack/mise.toml, and /home/jack/.t3/review-work/regrade-v10-rulings/assessors/homes/s-adm-sol/.cache/mise/ruby/ruby-build/bin/ruby-build; none was deliberately opened or used as evidence.
