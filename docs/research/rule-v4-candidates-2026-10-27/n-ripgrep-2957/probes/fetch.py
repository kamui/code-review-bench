import concurrent.futures, json, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
queries={
'pr-2957-review-comments':'repos/BurntSushi/ripgrep/pulls/2957/comments?per_page=100',
'pr-2957-issue-comments':'repos/BurntSushi/ripgrep/issues/2957/comments?per_page=100',
'pr-2957-reviews':'repos/BurntSushi/ripgrep/pulls/2957/reviews?per_page=100',
'pr-2957-commits':'repos/BurntSushi/ripgrep/pulls/2957/commits?per_page=100',
'issue-2956':'repos/BurntSushi/ripgrep/issues/2956',
'issue-2956-comments':'repos/BurntSushi/ripgrep/issues/2956/comments?per_page=100',
'later-releases':'repos/BurntSushi/ripgrep/releases?per_page=100',
'later-completion-commits':'repos/BurntSushi/ripgrep/commits?path=crates/core/flags/complete/rg.zsh&per_page=100',
}
searches={
'maintainers-test':'repo:BurntSushi/ripgrep \"test-complete\" created:<=2024-12-31',
'maintainers-compdef':'repo:BurntSushi/ripgrep compdef created:<=2024-12-31',
'maintainers-alias':'repo:BurntSushi/ripgrep zsh alias created:<=2024-12-31',
'maintainers-source':'repo:BurntSushi/ripgrep \"complete-zsh\" created:<=2024-12-31',
'later-test':'repo:BurntSushi/ripgrep \"test-complete\" created:>2024-12-31',
'later-alias':'repo:BurntSushi/ripgrep zsh alias created:>2024-12-31',
'later-compdef':'repo:BurntSushi/ripgrep compdef created:>2024-12-31',
}
code={
'public-code-M1':'\"_RG_COMPLETE_LIST_ARGS\"',
'public-code-zshenv':'filename:.zshenv compinit',
'public-code-M2':'\"alias -g ret\"',
'public-code-aliases':'\"alias -g\" path:.zshrc',
}
def fetch(item):
 name,args=item
 cmd=['gh','api']+args
 r=subprocess.run(cmd,capture_output=True,text=True)
 (ROOT/'upstream'/f'{name}.json').write_text(r.stdout)
 if r.stderr:(ROOT/'upstream'/f'{name}.stderr.txt').write_text(r.stderr)
 return dict(name=name,command=cmd,exit=r.returncode)
jobs=[(n,[q]) for n,q in queries.items()]
jobs +=[(n,['--method','GET','search/issues','-f','q='+q,'-f','per_page=100']) for n,q in searches.items()]
jobs +=[(n,['--method','GET','search/code','-f','q='+q,'-f','per_page=20']) for n,q in code.items()]
gql='query { repository(owner: \"BurntSushi\", name: \"ripgrep\") { pullRequest(number: 2957) { lastEditedAt userContentEdits(first:100) { totalCount nodes { editedAt diff } } } } }'
jobs.append(('pr-2957-graphql-edits',['graphql','-f','query='+gql]))
with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
 results=list(pool.map(fetch,jobs))
(ROOT/'upstream'/'fetch-log.json').write_text(json.dumps(results,indent=2)+'\n')
for r in results: print(r['name'],r['exit'])
