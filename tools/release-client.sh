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
