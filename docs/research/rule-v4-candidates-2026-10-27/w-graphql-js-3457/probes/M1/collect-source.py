import json
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
OUT = HERE.parents[1] / 'upstream'
CLONE = Path('/home/jack/.cache/rule-v4-candidates-scratch/w-graphql-js-3457/clone')
searches = {
 'project-docs': ['rg','-n','-i','validation|reference implementation|specification|block string|identical','README.md','docs'],
 'project-tests': ['rg','-n','identical fields with identical args|allows different order|block:|block string|StringValue','src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts','src/language/ast.ts','src/language/__tests__/parser-test.ts'],
 'source': ['rg','-n','-A','14','-B','4',r'StringValue:|case TokenKind.STRING|case TokenKind.BLOCK_STRING|interface StringValueNode|parseLiteral\(valueNode\)|parseLiteral\(ast\)|export function OverlappingFields','src/language/printer.ts','src/language/parser.ts','src/language/ast.ts','src/type/scalars.ts','src/validation/rules/OverlappingFieldsCanBeMergedRule.ts'],
}
records = []
for name,args in searches.items():
 result = subprocess.run(args, cwd=CLONE, capture_output=True, text=True)
 (OUT / (name + '.txt')).write_text(result.stdout + result.stderr)
 records.append({'name':name,'command':args,'exit':result.returncode,'lines':len(result.stdout.splitlines())})
for f in ['README.md','docs/APIReference-Validation.md','docs/GettingStarted.md','src/validation/validate.ts','src/validation/rules/OverlappingFieldsCanBeMergedRule.ts','src/utilities/sortValueNode.ts','src/jsutils/naturalCompare.ts']:
 source = CLONE / f
 if source.exists():
  (OUT / ('head-' + f.replace('/','--'))).write_bytes(source.read_bytes())
(OUT / 'source-search-log.json').write_text(json.dumps(records,indent=2)+'\n')
print(json.dumps(records,indent=2))
