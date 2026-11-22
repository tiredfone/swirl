#!/bin/sh
dir=${1:-/tmp/swirl/compare}
for p in $(ls $dir); do
  for i in 0 1 2; do
    a=$dir/$p/swirl-$i.png
    b=$dir/$p/chocolate-$i.png
    [ -f "$a" ] && [ -f "$b" ] || continue
    mean=$(convert "$a" "$b" -compose difference -composite -colorspace Gray -format '%[fx:mean*255]' info:)
    pct=$(convert "$a" "$b" -compose difference -composite -evaluate-sequence max -channel RGB -separate -evaluate-sequence max -threshold 6.27% -format '%[fx:mean*100]' info:)
    printf '%s/%s mean_abs_diff=%.2f pct_over_16=%.2f\n' "$p" "$i" "$mean" "$pct"
  done
done
