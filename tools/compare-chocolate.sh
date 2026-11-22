#!/bin/sh
src=${CHOCOLATE:-/var/www/html/chocolate}
sw=/var/www/swirl
for pair in \
  "shared/runtime.lua crates/swirl-lua/data/runtime.lua" \
  "shared/runtime2018.lua crates/swirl-lua/data/runtime2018.lua" \
  "corescripts2018/bundle.json data/server/corescripts2018/bundle.json" \
  "content/fonts/humanoidAnimateLocal.rbxm data/server/character/humanoidAnimateLocal.rbxm" \
  "content/fonts/humanoidSound.rbxm data/server/character/humanoidSound.rbxm" \
  "content/fonts/humanoidHealth.rbxm data/server/character/humanoidHealth.rbxm" \
  "content2018/avatar/scripts/humanoidAnimateLocalKeyframe.rbxm data/server/character/humanoidAnimateLocalKeyframe.rbxm"
do
    set -- $pair
    if cmp -s "$src/$1" "$sw/$2"; then
        echo "same      $2"
    else
        echo "different $2 (Chocolate $1)"
    fi
done
