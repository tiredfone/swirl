import re, sys
def norm_choc(path):
    out = []
    for l in open(path):
        l = l.rstrip('\n')
        m = re.match(r'^\[(error|warn|print)\] (?:(\S+?): )?(.*)$', l)
        if not m:
            continue
        kind, where, msg = m.groups()
        full = (where + ': ' if where else '') + msg
        msg = re.sub(r'^.*?:\d+: ', '', full) if kind == 'error' else re.sub(r'^[^:]*: ', '', full)
        out.append((kind, msg.strip()[:160]))
    return [(k, m.replace('Player1','TestingAccount')) for k, m in out]
def norm_swirl(path):
    out = []
    for l in open(path):
        l = l.rstrip('\n')
        m = re.match(r'^\[(error|warn|print)\] (.*)$', l)
        if not m:
            continue
        kind, msg = m.groups()
        msg = re.sub(r'^.*?:\d+: ', '', msg) if kind == 'error' else msg
        out.append((kind, msg.strip()[:160]))
    return [(k, m.replace('Player1','TestingAccount')) for k, m in out]
place = sys.argv[1]
c = norm_choc(f'/tmp/swirl/par/{place}.choc.log')
s = norm_swirl(f'/tmp/swirl/par/{place}.swirl.raw')
from collections import Counter
cc, sc = Counter(c), Counter(s)
print(f'chocolate {len(c)} lines, swirl {len(s)} lines')
for k in sorted(set(cc) | set(sc)):
    if cc[k] != sc[k]:
        print(f'  choc {cc[k]} swirl {sc[k]}: [{k[0]}] {k[1]}')
if cc == sc:
    print('identical script output')
