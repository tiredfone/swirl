#!/bin/sh
src=${CHOCOLATE:-/var/www/html/chocolate}
dst=/var/www/swirl/vendor/chocolate
bad=0
for d in client shared content corescripts2018; do
    if [ "$d" = corescripts2018 ]; then
        out=$(diff -rq "$src/$d/bundle.json" "$dst/$d/bundle.json" 2>&1 || true)
    else
        out=$(diff -rq "$src/$d" "$dst/$d" 2>&1 | grep -v "^Only in $src" || true)
    fi
    if [ -n "$out" ]; then
        echo "$out"
        bad=1
    fi
done
out=$(diff -rq "$src/content2018" "$dst/content2018" 2>&1 | grep -v "^Only in $src/content2018: platform" | grep -v "^Only in $src/content2018/platform" || true)
if [ -n "$out" ]; then
    echo "$out"
    bad=1
fi
if [ $bad = 1 ]; then
    echo "vendor/chocolate differs from $src; run tools/sync-chocolate.sh"
    exit 1
fi
echo "vendor/chocolate matches $src"
