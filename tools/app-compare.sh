#!/bin/sh
P=${1:-564}
OUT=${2:-/tmp/swirl/appcmp}
mkdir -p "$OUT"
cd /var/www/swirl
for MODE in js app; do
  if [ "$MODE" = app ]; then Q='&swirlapp=1'; else Q=''; fi
  (setsid ./target/release/swirl-server --port=4391 --place=$P --era=${ERA:-2012} --no-redeem > /tmp/swirl/srv.log 2>&1 &)
  sleep 3
  QS="$Q" SETTLE=${SETTLE:-12000} W=1100 H=700 timeout 120 node tools/swirl-gui-test.cjs 4391 "$OUT/$P-$MODE" 2>&1 | grep "vendored"
  pkill -x swirl-server
  sleep 2
done
cd "$OUT" && convert $P-js-1.png $P-app-1.png -compose difference -composite -colorspace Gray -format "$P mean=%[fx:mean*255]\n" info:
