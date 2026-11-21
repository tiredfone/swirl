#!/bin/sh
set -e
export PATH=$HOME/.cargo/bin:$PATH
cd /var/www/swirl
cargo build --release -p swirl-vanilla
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
checkscene() {
    name=$1
    shift
    out=$(tools/parity/scene.sh inst_$name "$@" 2>&1 | tail -1)
    echo "$name $out"
    mae=$(echo "$out" | sed -n 's/.*mae_rgba=\[\([0-9.]*\) \([0-9.]*\) \([0-9.]*\).*/\1 \2 \3/p')
    [ -n "$mae" ] || fail=1
    for v in $mae; do
        awk -v v="$v" 'BEGIN{exit !(v > 1.0)}' && fail=1
    done
    return 0
}
checkscene model model /var/www/assets/assets/1085 1024x1024 512x512
checkscene place place /var/www/assets/assets/564 1280x720 854x480
checkscene mesh mesh /var/www/assets/assets/1000 1024x1024 512x512
checkscene overlay place tools/parity/fixtures/overlay.rbxl 1280x720 854x480
mkdir -p /tmp/swirl/img
php tools/parity/image_gate.php /tmp/swirl/img
for t in webp gif bmp; do
    target/release/swirl image-png /tmp/swirl/img/t.$t /tmp/swirl/img/r_$t.png || fail=1
    out=$(target/release/swirl diff /tmp/swirl/img/g_$t.png /tmp/swirl/img/r_$t.png | tail -1)
    echo "image $t $out"
    echo "$out" | grep -q 'max=0 ' || fail=1
done
while read id sum; do
    got=""
    for f in /var/www/assets/assets/$id /var/www/assets/cache/$id; do
        [ -f "$f" ] || continue
        target/release/swirl mesh-convert $f /tmp/swirl/mc.out
        s=$(sha256sum /tmp/swirl/mc.out | cut -d' ' -f1)
        [ "$s" = "$sum" ] && got=ok
    done
    [ "$got" = ok ] || { echo "mesh-convert $id differs"; fail=1; }
done < tools/parity/mesh-golden.txt
echo "mesh-convert checked"
if [ "$fail" != 0 ]; then
    echo "parity check failed; binary not installed"
    exit 1
fi
cp target/release/swirl bin/.swirl.$$
mv bin/.swirl.$$ bin/swirl
echo "installed bin/swirl"
