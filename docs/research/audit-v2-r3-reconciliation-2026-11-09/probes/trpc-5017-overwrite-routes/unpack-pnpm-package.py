import json, os, sys, shutil, base64, binascii
store, index, out = sys.argv[1:4]
d = json.load(open(index))
print(d.get('name'), d.get('version'))
for rel, meta in d['files'].items():
    integ = meta['integrity']
    algo, b64 = integ.split('-', 1)
    hx = binascii.hexlify(base64.b64decode(b64)).decode()
    src = os.path.join(store, hx[:2], hx[2:])
    if meta.get('mode', 0) & 0o111:
        src += '-exec'
    dst = os.path.join(out, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copyfile(src, dst)
    os.chmod(dst, 0o755 if meta.get('mode', 0) & 0o111 else 0o644)
print('files', len(d['files']))
