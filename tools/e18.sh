cd /var/www/swirl
P=${1:-1969}
./target/release/swirl-server --port=4391 --place=$P --era=2018 --no-redeem > /tmp/swirl/srv18.log 2>&1 &
SP=$!
sleep 4
QS='&swirlapp=1' SETTLE=${SETTLE:-12000} EVAL='JSON.stringify({err:window.solarApp.errors.slice(0,3),cg:window.solarApp.vm.class_counts("CoreGui"),logs:(window.solarApp.logs||[]).slice(0,40)})' timeout 100 node tools/swirl-gui-test.cjs 4391 /tmp/swirl/gui/e18 2>&1 | grep "^\"{\|vendored\|pageerror" | python3 -c "
import sys,json,re
for l in sys.stdin:
    l=l.strip()
    if l.startswith('\"{'):
        d=json.loads(json.loads(l))
        print('err',d['err']); print('cg',d['cg'])
        seen={}
        for x in d['logs']:
            k=x['kind']+': '+x['msg'].split('\n')[0][:170]
            seen[k]=seen.get(k,0)+1
        for k,v in seen.items(): print(v,k)
    else: print(l)
"
kill $SP 2>/dev/null
