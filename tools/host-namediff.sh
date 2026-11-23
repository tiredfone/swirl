cd /var/www/swirl
P=${1:-564}
./target/release/swirl-server --port=4391 --place=$P --era=2012 --no-redeem > /tmp/swirl/srv.log 2>&1 &
SP=$!
sleep 3
QS='&swirlhost=1' SETTLE=${SETTLE:-14000} EVAL='(()=>{const L=window.solarLua;const js=[];const w=(i,p)=>{i.children.forEach(c=>{const n=p+"."+c.props.get("Name");if(c.ClassName===window.__cls)js.push(n);w(c,n)})};const out={};for(const cls of ["TextLabel","TextButton","Frame","ImageLabel","BoolValue"]){window.__cls=cls;js.length=0;w(L.world.dm.services.get("CoreGui"),"CoreGui");const rs=JSON.parse(window.solarVm.names_under("CoreGui",cls)).map(x=>x);const norm=(a)=>a.map(x=>x.replace(/^CoreGui\./,"")).sort();const A=norm(js),B=norm(rs);const cnt=(a)=>a.reduce((m,x)=>(m[x]=(m[x]||0)+1,m),{});const ca=cnt(A),cb=cnt(B);const only={js:[],rs:[]};for(const k of new Set([...A,...B])){const d=(ca[k]||0)-(cb[k]||0);if(d>0)only.js.push(k+" x"+d);else if(d<0)only.rs.push(k+" x"+-d)}out[cls]=only}return JSON.stringify(out)})()' timeout 100 node tools/swirl-gui-test.cjs 4391 /tmp/swirl/gui/l 2>&1 | grep '"{' | cut -c1-3000
kill $SP 2>/dev/null; wait $SP 2>/dev/null
