#!/bin/sh
set -e
cd /var/www/swirl
out=${OUT:-/var/www/swirl/dist/client}
place=${CHECK_PLACE:-564}
PROFILE=${PROFILE:-client-size} tools/build-client.sh "$out"
if [ ! -d "$out/chocolate" ]; then echo "build output has no chocolate directory"; exit 1; fi
version=$(sed -n 's/.*"version":"\([^"]*\)".*/\1/p' "$out/manifest.json")
notes=${NOTES:-"Swirl wasm client $version"}
if php /var/www/html/tools/release.php list developer | grep -q " $version "; then
    echo "developer $version already recorded"
else
    php /var/www/html/tools/release.php publish developer "$version" "$out" --entry=entry.js --notes="$notes"
fi
echo "movement parity:"
if ! DIST="$out" node tools/movement-parity.cjs "/tmp/swirl/mv/parity-$version.json"; then
    echo "movement parity failed for $version; not made current"
    exit 1
fi
echo "controller parity (Rust against Chocolate, R6 2012 and R15 2018):"
cargo build --release -p swirl-server --example move_trace > /dev/null 2>&1 || { echo "move_trace build failed"; exit 1; }
for v in r6 r15; do
    node tools/movement-spec-run.mjs /var/www/html/chocolate $v /tmp/swirl/mv/js-$v.json
    ./target/release/examples/move_trace tools/movement-spec.json $v /tmp/swirl/mv/rs-$v.json
done
if ! TOL=${SPEC_TOL:-0.001} node tools/movement-spec-compare.cjs; then
    echo "controller parity failed for $version; not made current"
    exit 1
fi
echo "2018 app path check (Esc menu text, Players row, walk animation):"
SWIRL_DEV_R15=1 ./target/release/swirl-server --port=4393 --place=3 --era=2018 --no-redeem > /tmp/swirl/mv/app2018-server.log 2>&1 &
APP2018_PID=$!
sleep 3
if ! DIST="$out" timeout 150 node tools/app2018-check.cjs 4393 app; then
    kill $APP2018_PID 2>/dev/null
    echo "2018 app path check failed for $version; not made current"
    exit 1
fi
kill $APP2018_PID 2>/dev/null
wait $APP2018_PID 2>/dev/null || true
echo "input check (right-drag camera, cursor, chat, player list; 2018 then 2012):"
SWIRL_DEV_R15=1 SWIRL_DEV_BADGE=/asset/?id=1 ./target/release/swirl-server --port=4393 --place=3 --era=2018 --no-redeem > /tmp/swirl/mv/input2018-server.log 2>&1 &
INPUT_PID=$!
sleep 3
if ! BADGE=1 DIST="$out" timeout 150 node tools/app-input-check.cjs 4393 2018; then
    kill $INPUT_PID 2>/dev/null
    echo "2018 input check failed for $version; not made current"
    exit 1
fi
kill $INPUT_PID 2>/dev/null
wait $INPUT_PID 2>/dev/null || true
SWIRL_DEV_R15=1 SWIRL_DEV_BADGE=/asset/?id=1 ./target/release/swirl-server --port=4393 --place=564 --era=2012 --no-redeem > /tmp/swirl/mv/input2012-server.log 2>&1 &
INPUT_PID=$!
sleep 3
if ! BADGE=1 DIST="$out" timeout 150 node tools/app-input-check.cjs 4393 2012; then
    kill $INPUT_PID 2>/dev/null
    echo "2012 input check failed for $version; not made current"
    exit 1
fi
kill $INPUT_PID 2>/dev/null
wait $INPUT_PID 2>/dev/null || true
if ! WALK=1 node tools/browser-check.cjs "$place" "$version" "/tmp/swirl/release-$version.png" > "/tmp/swirl/release-$version.json" 2>&1; then
    echo "browser check failed for $version, see /tmp/swirl/release-$version.json and .png; not made current"
    exit 1
fi
if ! FOCUS=${MESH_FOCUS:--20.33,5.7,-28.96,6} MIN_MESHES=1 node tools/browser-check.cjs "${MESH_PLACE:-972}" "$version" "/tmp/swirl/release-$version-mesh.png" > "/tmp/swirl/release-$version-mesh.json" 2>&1; then
    echo "mesh check failed for $version, see /tmp/swirl/release-$version-mesh.json and .png; not made current"
    exit 1
fi
echo "browser checks passed for $version (screenshots /tmp/swirl/release-$version.png and -mesh.png)"
if [ -n "$CURRENT" ]; then
    php /var/www/html/tools/release.php current developer "$version"
fi
