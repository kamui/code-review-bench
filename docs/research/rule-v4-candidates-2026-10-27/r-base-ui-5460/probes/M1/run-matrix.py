import difflib, json, os, subprocess
from pathlib import Path
out=Path(__file__).resolve().parent
src=Path('/home/jack/.cache/rule-v4-candidates-scratch/r-base-ui-5460/work/source')
path=src/'packages/react/src/field/control/FieldControl.tsx'
head=subprocess.check_output(['git','-C',str(src),'show','14d39e5d1:packages/react/src/field/control/FieldControl.tsx'],text=True)
def patch(m1=False,r6=False,r8=False):
 s=head
 if m1:
  s=s.replace('  useValueChanged(serializedValue, () => {', '  const preventionEventRef = React.useRef<React.SyntheticEvent | null>(null);\n\n  useValueChanged(serializedValue, () => {\n    const event = preventionEventRef.current;\n    preventionEventRef.current = null;\n    if (event && "baseUIHandlerPrevented" in event && event.baseUIHandlerPrevented) {\n      return;\n    }')
  s=s.replace('        onChange(event) {', '        onChangeCapture(event) {\n          preventionEventRef.current = event;\n          queueMicrotask(() => {\n            if (preventionEventRef.current === event) preventionEventRef.current = null;\n          });\n        },\n        onChange(event) {')
 if r6:
  s=s.replace('          // `validation.change` reads `markedDirtyRef`, so update dirty before validating.', '          if (details.isCanceled) return;\n\n          // `validation.change` reads `markedDirtyRef`, so update dirty before validating.')
 if r8:s=s.replace('if (serializedValue === undefined) {','if (serializedValue === undefined || disabled) {')
 return s
results=[]
for name,flags in [('m1-only',(True,False,False)),('r6-only',(False,True,False)),('m1-r6',(True,True,False)),('r8-only',(False,False,True)),('m1-r8',(True,False,True)),('all',(True,True,True))]:
 s=patch(*flags);path.write_text(s)
 (out/(name+'.patch')).write_text(''.join(difflib.unified_diff(head.splitlines(True),s.splitlines(True),fromfile='a/packages/react/src/field/control/FieldControl.tsx',tofile='b/packages/react/src/field/control/FieldControl.tsx')))
 cmd=['node','node_modules/vitest/vitest.mjs','run','--project','@base-ui/react','packages/react/src/field/control/ruling-M1.test.tsx']
 r=subprocess.run(cmd,cwd=src,env={**os.environ,'VITEST_ENV':'jsdom'},stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 (out/(name+'.txt')).write_text(r.stdout)
 obs=[json.loads(x.split('OBSERVATION ',1)[1]) for x in r.stdout.splitlines() if 'OBSERVATION ' in x]
 results.append({'variant':name,'exit_code':r.returncode,'observations':obs})
 print(name,r.returncode,flush=True)
path.write_text(head)
(out/'matrix.json').write_text(json.dumps(results,indent=2)+'\n')
