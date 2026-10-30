import concurrent.futures
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'upstream'
OUT.mkdir(exist_ok=True)

requests = {
    'pr-3457': ['repos/graphql/graphql-js/pulls/3457'],
    'pr-3457-comments': ['repos/graphql/graphql-js/issues/3457/comments?per_page=100'],
    'pr-3457-review-comments': ['repos/graphql/graphql-js/pulls/3457/comments?per_page=100'],
    'pr-3457-reviews': ['repos/graphql/graphql-js/pulls/3457/reviews?per_page=100'],
    'pr-3457-timeline': ['repos/graphql/graphql-js/issues/3457/timeline?per_page=100'],
    'pr-3457-commits': ['repos/graphql/graphql-js/pulls/3457/commits?per_page=100'],
    'releases': ['repos/graphql/graphql-js/releases?per_page=100'],
    'rule-history': ['repos/graphql/graphql-js/commits?path=src/validation/rules/OverlappingFieldsCanBeMergedRule.ts&per_page=100'],
    'maintainers-block': ['--method', 'GET', 'search/issues', '-f', 'q=repo:graphql/graphql-js "block string" "argument" created:<=2022-01-17T11:22:02Z', '-f', 'per_page=100'],
    'maintainers-printed': ['--method', 'GET', 'search/issues', '-f', 'q=repo:graphql/graphql-js "differing arguments" created:<=2022-01-17T11:22:02Z', '-f', 'per_page=100'],
    'maintainers-equivalent': ['--method', 'GET', 'search/issues', '-f', 'q=repo:graphql/graphql-js "identical" "string" created:<=2022-01-17T11:22:02Z', '-f', 'per_page=100'],
    'later-block': ['--method', 'GET', 'search/issues', '-f', 'q=repo:graphql/graphql-js "block string" "conflict"', '-f', 'per_page=100'],
    'later-printed': ['--method', 'GET', 'search/issues', '-f', 'q=repo:graphql/graphql-js "differing arguments" "string"', '-f', 'per_page=100'],
    'public-code': ['--method', 'GET', 'search/code', '-f', 'q="block string" "validate" "graphql" language:JavaScript', '-f', 'per_page=10'],
    'pr-3457-edits': ['graphql', '-f', 'query=query { repository(owner:"graphql", name:"graphql-js") { pullRequest(number:3457) { body lastEditedAt userContentEdits(first:100) { totalCount nodes { editedAt diff } } } } }'],
    'spec-validation': ['repos/graphql/graphql-spec/contents/spec/Section%205%20--%20Validation.md?ref=October2021'],
    'spec-language': ['repos/graphql/graphql-spec/contents/spec/Section%202%20--%20Language.md?ref=October2021'],
}

def fetch(item):
    name, args = item
    result = subprocess.run(['gh', 'api', *args], capture_output=True, text=True)
    (OUT / (name + '.json')).write_text(result.stdout)
    (OUT / (name + '.stderr.txt')).write_text(result.stderr)
    return {'name': name, 'command': ['gh', 'api', *args], 'exit': result.returncode}

if __name__ == '__main__':
    with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
        records = list(pool.map(fetch, requests.items()))
    (OUT / 'fetch-log.json').write_text(json.dumps(records, indent=2) + '\n')
    print(json.dumps(records, indent=2))
