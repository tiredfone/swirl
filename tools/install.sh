#!/bin/sh
set -e
export PATH=$HOME/.cargo/bin:$PATH
cd /var/www/swirl
cargo build --release
mkdir -p bin /tmp/swirl
fail=0
check() {
    name=$1
    shift
    out=$(tools/parity/run.sh inst_$name "$@" 2>&1 | tail -1)
    echo "$name $out"
    mae=$(echo "$out" | sed -n 's/.*mae_rgba=\[\([0-9.]*\) \([0-9.]*\) \([0-9.]*\).*/\1 \2 \3/p')
    [ -n "$mae" ] || fail=1
    for v in $mae; do
        awk -v v="$v" 'BEGIN{exit !(v > 0.5)}' && fail=1
    done
    return 0
}
check plain --colors=194,194,194,194,194,194
check clothes --ids=1287,1260,1005,1011
check pkg --ids=1106
check r15 --rig=R15 --pose=507766388 --ids=1106,1005
check item --item=1011 --size=512x512
if [ "$fail" != 0 ]; then
    echo "parity check failed; binary not installed"
    exit 1
fi
cp target/release/swirl-vanilla bin/.swirl-vanilla.$$
mv bin/.swirl-vanilla.$$ bin/swirl-vanilla
echo "installed bin/swirl-vanilla"
