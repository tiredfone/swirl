cd /var/www/swirl
P=${1:-564}
(setsid ./target/release/swirl-server --port=4391 --place=$P --era=2012 --no-redeem > /tmp/swirl/srv.log 2>&1 &)
sleep 3
QS='&swirlhost=1' SETTLE=${SETTLE:-12000} EVAL='(()=>{const L=window.solarLua;const g=L.state.gui;const rs=JSON.parse(window.solarVm.text_bounds_all());const js=new Map();const w=(i,p)=>{i.children.forEach(c=>{const n=p?p+"."+c.props.get("Name"):c.props.get("Name");if(c.isA("TextLabel")||c.isA("TextButton")||c.isA("TextBox")){const b=g.textBounds(c);const k=n+"|"+c.props.get("Text");(js.get(k)||js.set(k,[]).get(k)).push(b)}w(c,n)})};w(L.world.dm.root,"");let n=0,bad=[],worst=0;for(const [name,t,bw,bh] of rs){const k=name+"|"+t;const l=js.get(k);if(!l||!l.length){continue}const b=l.shift();n++;const d=Math.max(Math.abs(b[0]-bw),Math.abs(b[1]-bh));worst=Math.max(worst,d);if(d>0.01&&bad.length<5)bad.push([k.slice(0,70),b,[bw,bh]])}return JSON.stringify({rs:rs.length,matched:n,worst,bad})})()' timeout 100 node tools/swirl-gui-test.cjs 4391 /tmp/swirl/gui/l 2>&1 | grep '"{' | cut -c1-1200
pkill -x swirl-server
