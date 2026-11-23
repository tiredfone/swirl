cd /var/www/swirl
P=${1:-564}
OUT=${2:-/tmp/swirl/guistyles}
mkdir -p $OUT
for MODE in js rust; do
  if [ $MODE = rust ]; then Q='&swirlhost=1&swirlgui=1'; else Q='&swirlhost=1'; fi
  (setsid ./target/release/swirl-server --port=4391 --place=$P --era=2012 --no-redeem > /tmp/swirl/srv.log 2>&1 &)
  sleep 3
  QS="$Q" SETTLE=${SETTLE:-10000} MODE=$MODE EVAL='(()=>{const L=window.solarLua;const props=["left","top","width","height","display","zIndex","background","overflow","boxShadow","color","fontSize","fontWeight","textShadow","transform","opacity","fontFamily","whiteSpace","textAlign","justifyContent","alignItems"];const out={};if(!window.solarVm||true){}const mode=location.search.includes("swirlgui")?"rust":"js";if(mode==="js"){for(const [inst,n] of L.state.gui.nodes){const p=[];for(let x=inst;x&&x.parent;x=x.parent)p.unshift(x.props.get("Name"));const k=p.join(".");const o={};for(const q of props)o[q]=n.el.style[q];o.text=n.text?n.text.textContent:null;out[k]=(out[k]||[]).concat([o])}}else{document.querySelectorAll("[data-ipath]").forEach(e=>{const k=e.getAttribute("data-ipath");const o={};for(const q of props)o[q]=e.style[q];const t=e.querySelector(":scope > .ch-g-text");o.text=t?t.textContent:null;out[k]=(out[k]||[]).concat([o])})}return JSON.stringify(out)})()' timeout 100 node tools/swirl-gui-test.cjs 4391 /tmp/swirl/gui/l 2>&1 | grep '^"{' > $OUT/$MODE.txt
  pkill -x swirl-server
  sleep 2
done
python3 - <<'PY'
import json,sys
def load(p):
    s=open(p).read().strip()
    return json.loads(json.loads(s))
a=load('/tmp/swirl/guistyles/js.txt'); b=load('/tmp/swirl/guistyles/rust.txt')
print('nodes js',len(a),'rust',len(b))
per={}; sample=[]
for k,l in a.items():
    r=b.get(k)
    if not r: 
        per['missing']=per.get('missing',0)+1; continue
    for i,o in enumerate(l):
        if i>=len(r): break
        for q,v in o.items():
            if v!=r[i].get(q):
                per[q]=per.get(q,0)+1
                if len(sample)<14: sample.append((k[-45:],q,v,r[i].get(q)))
print(per)
for s in sample: print(s)
PY
