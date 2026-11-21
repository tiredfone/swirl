#!/bin/sh
name=$1
kind=$2
file=$3
size=$4
outsize=$5
era=${6:-2012}
export PATH=$HOME/.cargo/bin:$PATH
out=/tmp/swirl
mkdir -p $out
php /var/www/swirl/tools/parity/php_scene.php --kind=$kind --file=$file --size=$size --outsize=$outsize --era=$era --out=$out/$name.php.png 2>&1 | grep -v libpng
/var/www/swirl/target/release/swirl-vanilla --scene=$kind --scene-file=$file --scene-size=$size --out-size=$outsize --era=$era --out=$out/$name.swirl.png 2>&1
/var/www/swirl/target/release/swirl-vanilla diff $out/$name.php.png $out/$name.swirl.png $out/$name.diff.png
