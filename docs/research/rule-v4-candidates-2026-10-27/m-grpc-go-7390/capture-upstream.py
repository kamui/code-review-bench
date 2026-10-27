import json, subprocess, concurrent.futures
from pathlib import Path
D=Path(__file__).parent/'upstream'
queries={
'pr-comments':'repos/grpc/grpc-go/issues/7390/comments?per_page=100',
'pr-reviews':'repos/grpc/grpc-go/pulls/7390/reviews?per_page=100',
'pr-review-comments':'repos/grpc/grpc-go/pulls/7390/comments?per_page=100',
'issue-7365':'repos/grpc/grpc-go/issues/7365',
'issue-7365-comments':'repos/grpc/grpc-go/issues/7365/comments?per_page=100',
'backoff-spec-history':'repos/grpc/grpc/commits?path=doc/connection-backoff.md&until=2024-07-09T06:40:48Z&per_page=1',
'maintainers-backoff':'search/issues?q=repo%3Agrpc%2Fgrpc-go+backoff+created%3A%3C%3D2024-07-09&per_page=100',
'maintainers-update':'search/issues?q=repo%3Agrpc%2Fgrpc-go+%22UpdateAddresses%22+backoff+created%3A%3C%3D2024-07-09&per_page=100',
'later-backoff':'search/issues?q=repo%3Agrpc%2Fgrpc-go+%22backoff%22+%22cancel%22&per_page=100',
'public-code':'search/code?q=%22WithConnectParams%22+%22Multiplier%22+language%3AGo+-repo%3Agrpc%2Fgrpc-go&per_page=10',
'release-v1.66.0':'repos/grpc/grpc-go/releases/tags/v1.66.0'
}
def fetch(item):
 name,endpoint=item
 r=subprocess.run(['gh','api',endpoint],capture_output=True,text=True)
 (D/(name+'.json')).write_text(r.stdout)
 return dict(name=name,command=['gh','api',endpoint],exit=r.returncode,stderr=r.stderr)
with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool: receipts=list(pool.map(fetch, queries.items()))
query='query { repository(owner:"grpc",name:"grpc-go") { pullRequest(number:7390) { lastEditedAt userContentEdits(first:100) { nodes { editedAt diff } pageInfo { hasNextPage } } } } }'
r=subprocess.run(['gh','api','graphql','-f','query='+query],capture_output=True,text=True)
(D/'pr-description-edits.json').write_text(r.stdout)
receipts.append(dict(name='pr-description-edits',command=['gh','api','graphql','-f','query='+query],exit=r.returncode,stderr=r.stderr))
(D/'capture-receipts.json').write_text(json.dumps(receipts,indent=2)+'\n')
print(json.dumps(receipts,indent=2))
