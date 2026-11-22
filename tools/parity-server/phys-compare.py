import json,sys,math
a=json.load(open(sys.argv[1])); b=json.load(open(sys.argv[2]))
n=min(len(a),len(b))
worst=0;worst_at=None;first_bad=None
per={}
for t in range(n):
    ra={r[0]:r for r in a[t]}; rb={r[0]:r for r in b[t]}
    for name in ra:
        if name not in rb: print('missing',name); sys.exit(1)
        d=max(abs(x-y) for x,y in zip(ra[name][1],rb[name][1]))
        dv=max(abs(x-y) for x,y in zip(ra[name][2],rb[name][2]))
        per[name]=max(per.get(name,0),d)
        if d>worst: worst=d;worst_at=(t,name)
        if d>1e-3 and first_bad is None: first_bad=(t,name,d)
print('ticks',n,'parts',len(a[0]),'worst cframe diff',worst,'at',worst_at,'first >1e-3',first_bad)
print({k:round(v,5) for k,v in per.items()})
