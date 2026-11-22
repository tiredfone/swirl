#!/bin/sh
# usage: swirl-run.sh place era port ms
P=$1; ERA=$2; PORT=$3; MS=$4
cd /var/www/swirl
target/release/swirl-server --place=$P --port=$PORT --era=$ERA --no-redeem > /tmp/swirl/par/$P.swirl.raw 2>&1 &
SP=$!
sleep 2
node -e "
const WebSocket=require('/tmp/claude-1000/pp/node_modules/ws');
const ws=new WebSocket('ws://127.0.0.1:$PORT/ws?ticket=dev');
const c={};ws.on('message',d=>{const m=JSON.parse(d.toString());c[m.t]=(c[m.t]||0)+1});
setTimeout(()=>{console.log(JSON.stringify(c));ws.close();setTimeout(()=>process.exit(0),200)},$MS);
"
kill $SP
