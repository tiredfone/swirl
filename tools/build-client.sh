#!/bin/sh
set -e
export PATH=$HOME/.cargo/bin:$PATH
cd /var/www/swirl
profile=${PROFILE:-client}
out=${1:-/var/www/swirl/dist/client}
version=$(grep -m1 "^version" crates/swirl-client/Cargo.toml | sed "s/.*\"\([0-9]*\.[0-9]*\)\.[0-9]*\".*/\1/")
short=$(git rev-parse --short HEAD 2>/dev/null || echo unknown)
sh tools/check-chocolate-drift.sh >&2 || echo "warning: vendor/chocolate has drifted from Chocolate" >&2
[ -x /var/www/swirl/.toolchain/wasi-sdk-34.0-x86_64-linux/bin/clang ] || sh tools/setup-toolchain.sh
cargo build --target wasm32-unknown-unknown --profile $profile -p swirl-client
rm -rf "$out.tmp"
mkdir -p "$out.tmp"
wasm-bindgen --target web --no-typescript --out-dir "$out.tmp" target/wasm32-unknown-unknown/$profile/swirl_client.wasm
node tools/patch-wasi.mjs "$out.tmp/swirl_client.js"
if [ -n "$WASM_OPT" ] && command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -O1 "$out.tmp/swirl_client_bg.wasm" -o "$out.tmp/swirl_client_bg.wasm.opt" && mv "$out.tmp/swirl_client_bg.wasm.opt" "$out.tmp/swirl_client_bg.wasm"
fi
cp crates/swirl-client/entry.js "$out.tmp/entry.js"
cp crates/swirl-client/lua.js "$out.tmp/lua.js"
cp crates/swirl-client/pack.js "$out.tmp/pack.js"
node tools/pack-vendor.mjs vendor/chocolate "$out.tmp/chocolate"
cat > "$out.tmp/manifest.json" <<JSON
{"version":"$version","era":[2012,2018],"entry":"entry.js","build":"$short","profile":"$profile"}
JSON
rm -rf "$out"
mv "$out.tmp" "$out"
echo "built $out version $version ($short)"
