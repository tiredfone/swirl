import json, sys
a = json.load(open(sys.argv[1]))['snap']
b = json.load(open(sys.argv[2]))['snap']
def canon(snap):
    byid = {r[0]: r for r in snap}
    def path(i):
        r = byid[i]
        p = dict(zip(r[3][::2], r[3][1::2])).get('Name', r[1])
        return (path(r[2]) + '/' if r[2] in byid else '') + r[1] + ':' + str(p)
    out = {}
    refs = {}
    for r in snap:
        props = dict(zip(r[3][::2], r[3][1::2]))
        out.setdefault(path(r[0]), []).append((r[1], props))
    return out, byid, path
ca, ia, pa = canon(a)
cb, ib, pb = canon(b)
def norm(props, byid, path):
    o = {}
    for k, v in props.items():
        if k in ('CurrentCamera',) or (isinstance(v, int) and k.endswith('Camera')):
            v = path(v) if v in byid else v
        o[k] = v
    return o
diff = 0
for k in sorted(set(ca) | set(cb)):
    la = [norm(p, ia, pa) for _, p in ca.get(k, [])]
    lb = [norm(p, ib, pb) for _, p in cb.get(k, [])]
    if la != lb:
        diff += 1
        if diff <= 8:
            print('DIFF', k)
            print('  JS', json.dumps(la)[:400])
            print('  SW', json.dumps(lb)[:400])
print('records', len(a), len(b), 'diffs', diff)
