import concurrent.futures
import json
import pathlib
import subprocess

OUT = pathlib.Path(__file__).parent / 'upstream'
queries = {
    'pr-comments.json': ['repos/graphql/graphql-js/pulls/1582/comments?per_page=100'],
    'issue-comments.json': ['repos/graphql/graphql-js/issues/1582/comments?per_page=100'],
    'pr-commits.json': ['repos/graphql/graphql-js/pulls/1582/commits?per_page=100'],
    'pr-reviews.json': ['repos/graphql/graphql-js/pulls/1582/reviews?per_page=100'],
    'releases.json': ['repos/graphql/graphql-js/releases?per_page=100'],
    'later-test-changes.json': ['repos/graphql/graphql-js/commits?path=src/error/__tests__/GraphQLError-test.js&per_page=100'],
}
searches = {
    'maintainers-location.json': ('issues', 'repo:graphql/graphql-js GraphQLError locations created:<=2018-11-21T14:25:49Z'),
    'maintainers-serialization.json': ('issues', 'repo:graphql/graphql-js serializes created:<=2018-11-21T14:25:49Z'),
    'maintainers-first-line.json': ('issues', 'repo:graphql/graphql-js "first line" created:<=2018-11-21T14:25:49Z'),
    'later-location.json': ('issues', 'repo:graphql/graphql-js "GraphQLError" "column" created:>2018-11-21T14:25:49Z'),
    'later-serialization.json': ('issues', 'repo:graphql/graphql-js "serializes to include message and locations"'),
    'public-code.json': ('code', '"parse(\'{ field }\')" "GraphQLError" -repo:kamui/code-review-bench'),
}
for name, (kind, query) in searches.items():
    queries[name] = ['-X', 'GET', 'search/' + kind, '-f', 'q=' + query, '-f', 'per_page=100']

def capture(item):
    name, args = item
    command = ['gh', 'api', *args]
    result = subprocess.run(command, capture_output=True, text=True)
    (OUT / name).write_text(result.stdout)
    (OUT / (name + '.stderr')).write_text(result.stderr)
    return {'saved': name, 'command': command, 'exit': result.returncode}

with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
    records = list(pool.map(capture, queries.items()))
(OUT / 'capture-commands.json').write_text(json.dumps(records, indent=2) + '\n')
print(json.dumps(records, indent=2))
