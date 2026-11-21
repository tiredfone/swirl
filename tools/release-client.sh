#!/bin/sh
set -e
cd /var/www/swirl
out=${OUT:-/var/www/swirl/dist/client}
tools/build-client.sh "$out"
version=$(sed -n 's/.*"version":"\([^"]*\)".*/\1/p' "$out/manifest.json")
notes=${NOTES:-"Swirl wasm client $version"}
php /var/www/html/tools/release.php publish developer "$version" "$out" --entry=entry.js --notes="$notes" ${CURRENT:+--current}
