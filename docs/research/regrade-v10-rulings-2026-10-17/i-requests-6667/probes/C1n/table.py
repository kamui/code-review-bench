import re,glob,os,sys
files=['result-base.txt','result-head.txt']+sorted(glob.glob('results/fix-*.txt'))
tab={}
for f in files:
    sec=None
    for line in open(f):
        if line.startswith('## '): sec=line[3:].strip()
        elif line.startswith('- '):
            k,_,v=line[2:].partition(': ')
            if not v: k,v='value',k
            v=re.sub(r'\s+\[raised via.*','',v.strip()); v=re.sub(r'invalid path: \S+','invalid path: <default>',v); v=re.sub(r"port=\d+","port=N",v)
            tab.setdefault((sec,k),{})[os.path.basename(f)]=v
for (s,k),d in tab.items():
    print('*',s,'|',k)
    for f in files: print('   ',os.path.basename(f).ljust(28),(d.get(os.path.basename(f)) or '')[:90])
