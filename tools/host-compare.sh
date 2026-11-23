cd /var/www/swirl
P=${1:-564}
(setsid ./target/release/swirl-server --port=4391 --place=$P --era=2012 --no-redeem > /tmp/swirl/srv.log 2>&1 &)
sleep 3
QS='&swirlhost=1' SETTLE=${SETTLE:-12000} EVAL='(()=>{const L=window.solarLua;const cnt=(r)=>{const m={};const w=(i)=>{i.children.forEach(c=>{m[c.ClassName]=(m[c.ClassName]||0)+1;w(c)})};w(r);return m};const out={};for(const s of ["CoreGui"]){const js=cnt(L.world.dm.services.get(s));const rs=JSON.parse(window.solarVm.class_counts(s));const diff={};for(const k of new Set([...Object.keys(js),...Object.keys(rs)])) if((js[k]||0)!==(rs[k]||0)) diff[k]=[js[k]||0,rs[k]||0];out[s]=diff}out.rsLogs=(window.solarRustLogs||[]).filter(l=>l.kind==="error").map(l=>l.msg.split("\n")[0].slice(0,160));return JSON.stringify(out)})()' timeout 100 node tools/swirl-gui-test.cjs 4391 /tmp/swirl/gui/l 2>&1 | grep '"{' | cut -c1-1200
pkill -x swirl-server
