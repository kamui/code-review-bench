"""Usage: mkvariant.py <name> <kind> ; builds variants/G4/<name> from trees/head and writes patches/G4/<name>.patch"""
import sys, shutil, subprocess, pathlib, re
name, kind = sys.argv[1], sys.argv[2]
root = pathlib.Path('variants/G4')/name
if root.exists(): shutil.rmtree(root)
shutil.copytree('trees/head/packages', root/'packages', symlinks=True)
U = root/'packages/server/src/core/internals/utils.ts'
MW = root/'packages/server/src/core/middleware.ts'
PB = root/'packages/server/src/core/internals/procedureBuilder.ts'
s = U.read_text()
start = s.index('export type Overwrite<TType, TWith>'); end = s.index('/**', start)
head_def = s[start:end]
MERGE = """{
        [K in keyof TType | keyof TWith]: K extends keyof TWith
          ? TWith[K]
          : K extends keyof TType
          ? TType[K]
          : never;
      }"""
def setdef(body): 
    global s
    s = s[:start] + body + "\n\n" + s[end:]
BASE_DEF = f"""export type OverwriteBefore<TType, TWith> = TType extends any
  ? TWith extends any
    ? {MERGE}
    : never
  : never;

"""
def site(path, pattern, count_expected=None, only=None):
    t = path.read_text()
    idx = [m.start() for m in re.finditer(r'(?<![A-Za-z])Overwrite<', t)]
    # skip the definition itself
    idx = [i for i in idx if not t[max(0,i-12):i].endswith('export type ')]
    chosen = idx if only is None else [idx[k] for k in only]
    for i in sorted(chosen, reverse=True):
        t = t[:i] + 'OverwriteBefore<' + t[i+len('Overwrite<'):]
    if path != U:
        t = t.replace("  Overwrite,\n", "  Overwrite,\n  OverwriteBefore,\n", 1)
    path.write_text(t)
    return len(idx)
if kind == 'only-first-check':
    setdef(f"export type Overwrite<TType, TWith> = TType extends object\n  ? TWith extends any\n    ? {MERGE}\n    : never\n  : never;")
elif kind == 'only-second-check':
    setdef(f"export type Overwrite<TType, TWith> = TType extends any\n  ? TWith extends object\n    ? {MERGE}\n    : never\n  : never;")
elif kind == 'wrap':
    inner = head_def.replace('export type Overwrite<TType, TWith> = ', '').rstrip().rstrip(';')
    inner = '\n'.join('      ' + l if l else l for l in inner.split('\n'))
    setdef(f"export type Overwrite<TType, TWith> = TType extends any\n  ? TWith extends any\n    ? (\n{inner}\n      )\n    : never\n  : never;")
elif kind.startswith('site-'):
    s = s[:start] + BASE_DEF + s[start:]
    U.write_text(s)
    which = kind[5:]
    if which == 'procedure-builder': print('sites', site(PB, None))
    elif which == 'middleware': print('sites', site(MW, None))
    elif which == 'resolve-options':
        t = U.read_text(); a = "    Overwrite<TParams['_config']['$types']['ctx'], TParams['_ctx_out']>"; assert a in t
        U.write_text(t.replace(a, a.replace('Overwrite<', 'OverwriteBefore<')))
    elif which.startswith('middleware-'):
        print('sites', site(MW, None, only=[int(which.split('-')[1])]))
    s = None
if s is not None: U.write_text(s)
files = subprocess.run(['diff', '-rq', 'trees/head/packages/server/src', str(root/'packages/server/src')], capture_output=True, text=True).stdout
parts = []
for line in files.split('\n'):
    m = re.match(r'Files trees/head/(\S+) and \S+ differ', line)
    if not m: continue
    rel = m.group(1)
    parts.append(subprocess.run(['diff', '-u', '--label', 'a/' + rel, '--label', 'b/' + rel, 'trees/head/' + rel, str(root/rel)], capture_output=True, text=True).stdout)
pathlib.Path(f'patches/G4/{name}.patch').write_text(''.join(parts))
