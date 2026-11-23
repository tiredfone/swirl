#!/bin/sh
P=${1:-564}
OUT=${2:-/tmp/swirl/guicmp}
mkdir -p "$OUT"
cd /var/www/swirl
for MODE in js rust; do
  if [ "$MODE" = rust ]; then Q='&swirlhost=1&swirlgui=1'; else Q='&swirlhost=1'; fi
  (setsid ./target/release/swirl-server --port=4391 --place=$P --era=${ERA:-2012} --no-redeem > /tmp/swirl/srv.log 2>&1 &)
  sleep 3
  QS="$Q" SETTLE=${SETTLE:-12000} W=${W:-1100} H=${H:-700} EVAL='JSON.stringify({nodes:window.solarRustGuiNodes||0,err:window.solarLua.errors.slice(0,3)})' timeout 100 node tools/swirl-gui-test.cjs 4391 "$OUT/$MODE" 2>&1 | grep '"{'
  pkill -x swirl-server
  sleep 2
done
sh tools/pair-diff.sh "$OUT/js-1.png" "$OUT/rust-1.png"
