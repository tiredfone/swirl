#!/bin/sh
set -e
cp /var/www/swirl/vendor/chocolate/shared/runtime.lua /var/www/swirl/crates/swirl-lua/data/runtime.lua
cp /var/www/swirl/vendor/chocolate/shared/runtime2018.lua /var/www/swirl/crates/swirl-lua/data/runtime2018.lua
node /var/www/swirl/tools/dump-luadata.mjs > /var/www/swirl/crates/swirl-lua/data/luadata.lua
echo "runtime.lua and luadata.lua synced"
