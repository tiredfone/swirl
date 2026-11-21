#!/bin/sh
name="$1"
shift
export PATH=$HOME/.cargo/bin:$PATH
out=/tmp/swirl
mkdir -p $out
here=$(dirname "$0")
bin=/var/www/swirl/target/release/swirl
args="$*"
php $here/php_render.php --out=$out/$name.php.png $args 2>&1 | grep -v libpng
$bin --out=$out/$name.swirl.png $args 2>&1 | grep -v "^$"
if [ -n "$PARITY_EXACT" ]; then
    php $here/php_render.php --out=$out/$name.phpexact.png --exact=1 $args 2>&1 | grep -v libpng
    echo "vs exact:"; $bin diff $out/$name.phpexact.png $out/$name.swirl.png $out/$name.diffexact.png
fi
$bin diff $out/$name.php.png $out/$name.swirl.png $out/$name.diff.png
