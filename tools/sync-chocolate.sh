#!/bin/sh
set -e
src=${CHOCOLATE:-/var/www/html/chocolate}
dst=/var/www/swirl/vendor/chocolate
mkdir -p "$dst"
for d in client shared content content2018 corescripts2018; do
    mkdir -p "$dst/$d"
done
rsync -a --delete "$src/client/" "$dst/client/"
rsync -a --delete "$src/shared/" "$dst/shared/"
rsync -a --delete "$src/content/" "$dst/content/"
rsync -a --delete --exclude=/platform/ "$src/content2018/" "$dst/content2018/"
cp "$src/corescripts2018/bundle.json" "$dst/corescripts2018/bundle.json"
cp "$src/corescripts2018/LICENSE.txt" "$src/corescripts2018/SOURCE.txt" "$dst/corescripts2018/"
cp "$src/shared/runtime.lua" /var/www/swirl/crates/swirl-lua/data/runtime.lua
cp "$src/shared/runtime2018.lua" /var/www/swirl/crates/swirl-lua/data/runtime2018.lua
node /var/www/swirl/tools/dump-luadata.mjs > /var/www/swirl/crates/swirl-lua/data/luadata.lua
echo '{"type":"module","private":true}' > "$dst/package.json"
echo "synced $src into $dst"
