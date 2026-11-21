#!/bin/sh
set -e
cd /var/www/swirl
out=${OUT:-/var/www/swirl/dist/client}
place=${CHECK_PLACE:-564}
tools/build-client.sh "$out"
version=$(sed -n 's/.*"version":"\([^"]*\)".*/\1/p' "$out/manifest.json")
notes=${NOTES:-"Swirl wasm client $version"}
if php /var/www/html/tools/release.php list developer | grep -q " $version "; then
    echo "developer $version already recorded"
else
    php /var/www/html/tools/release.php publish developer "$version" "$out" --entry=entry.js --notes="$notes"
fi
if ! node tools/browser-check.cjs "$place" "$version" "/tmp/swirl/release-$version.png" > "/tmp/swirl/release-$version.json" 2>&1; then
    echo "browser check failed for $version, see /tmp/swirl/release-$version.json and .png; not made current"
    exit 1
fi
echo "browser check passed for $version (screenshot /tmp/swirl/release-$version.png)"
if [ -n "$CURRENT" ]; then
    php /var/www/html/tools/release.php current developer "$version"
fi
