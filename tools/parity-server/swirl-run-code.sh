#!/bin/bash
# usage: swirl-run-code.sh place era port luafile [env assignments via environment]
P=$1; ERA=$2; PORT=$3; LUA=$4
cd /var/www/swirl
echo -n parity-key > /tmp/swirl/key.txt
target/release/swirl-server --place=$P --era=$ERA --port=$PORT --no-redeem --redeem-key-file=/tmp/swirl/key.txt > /tmp/swirl/par/$P.run.log 2>&1 &
SP=$!
sleep 3
node -e "
const WebSocket=require('/tmp/claude-1000/pp/node_modules/ws');
const http=require('http');
const ws=new WebSocket('ws://127.0.0.1:$PORT/ws?ticket=dev');
setTimeout(()=>{
 http.get('http://127.0.0.1:$PORT/status',r=>{let b='';r.on('data',d=>b+=d);r.on('end',()=>{
  const st=JSON.parse(b);const job=(st.places||st.instances||[])[0]?.job||(st.places||st.instances||[])[0]?.jobId;
  const fs=require('fs');
  http.request({host:'127.0.0.1',port:$PORT,path:'/run',method:'POST',headers:{'X-Chocolate-Key':'parity-key','Content-Type':'application/json'}},q=>{let o='';q.on('data',d=>o+=d);q.on('end',()=>{const j=JSON.parse(o);console.log((j.output||[]).join('\\n')||o)})}).end(JSON.stringify({source:fs.readFileSync('$LUA','utf8'),by:'t',job}));
 })});
},1500);
setTimeout(()=>process.exit(0),5000)"
kill $SP
