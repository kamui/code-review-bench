import json, subprocess, pathlib, concurrent.futures
out=pathlib.Path(__file__).parent
jobs={
"pr-comments": ["repos/django/django/issues/17914/comments?per_page=100"],
"pr-review-comments": ["repos/django/django/pulls/17914/comments?per_page=100"],
"pr-reviews": ["repos/django/django/pulls/17914/reviews?per_page=100"],
"pr-timeline": ["repos/django/django/issues/17914/timeline?per_page=100"],
"old-pr": ["repos/django/django/pulls/17594"],
"old-comments": ["repos/django/django/pulls/17594/comments?per_page=100"],
"later-pr-18498": ["repos/django/django/pulls/18498"],
"later-pr-18498-comments": ["repos/django/django/pulls/18498/comments?per_page=100"],
"release-5.1": ["repos/django/django/contents/docs/releases/5.1.txt?ref=5.1"],
"psycopg-pool-doc-commits": ["repos/psycopg/psycopg/commits?path=docs/advanced/pool.rst&until=2024-03-01T08:02:02Z&per_page=1"],
"psycopg-api-doc-commits": ["repos/psycopg/psycopg/commits?path=docs/api/pool.rst&until=2024-03-01T08:02:02Z&per_page=1"],
"psycopg-conn-doc-commits": ["repos/psycopg/psycopg/commits?path=docs/api/connections.rst&until=2024-03-01T08:02:02Z&per_page=1"],
"description-edits": ["graphql", "-f", 'query=query { repository(owner:"django",name:"django") { pullRequest(number:17914) { body userContentEdits(first:100) { totalCount nodes { editedAt diff } } } } }'],
}
queries={
"M1-maintainers": 'repo:django/django "isolation_level" created:<=2024-03-01',
"M2-maintainers": 'repo:django/django "get_new_connection" created:<=2024-03-01',
"M3-maintainers": 'repo:django/django "TIME_ZONE" "pool" created:<=2024-03-01',
"M4-maintainers": 'repo:django/django "close_pool" created:<=2024-03-01',
"P1-maintainers": 'repo:django/django "cannot drop the currently open database" created:<=2024-03-01',
"later-pool": 'repo:django/django "pool" "timezone"',
"later-close": 'repo:django/django "close_pool"',
"later-leak": 'repo:django/django "pool" "isolation_level"',
"later-drop": 'repo:django/django "cannot drop the currently open database"',
}
for n,q in queries.items(): jobs[n]=["-X","GET","search/issues","-f","q="+q,"-f","per_page=100"]
for n,q in {"M1":'"isolation_level" "getconn" language:Python',"M2":'"get_new_connection" "conn_params" language:Python',"M3":'"override_settings" "TIME_ZONE" "pool" language:Python',"M4":'"close_pool" "ensure_timezone" language:Python',"P1":'"_nodb_cursor" language:Python'}.items(): jobs[n+"-public-code"]=["-X","GET","search/code","-f","q="+q,"-f","per_page=10"]
def run(item):
 n,args=item; r=subprocess.run(["gh","api"]+args,capture_output=True,text=True)
 (out/(n+".json")).write_text(r.stdout)
 (out/(n+".command.txt")).write_text(' '.join(["gh","api"]+args)+'\nexit='+str(r.returncode)+'\n'+r.stderr)
 try:d=json.loads(r.stdout);return n,r.returncode,d.get("total_count") if isinstance(d,dict) else len(d)
 except:return n,r.returncode,r.stdout[:100]
with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
 for r in pool.map(run,jobs.items()): print(r)
