#!/bin/sh
args=""
for a in "$@"; do
    case "$a" in
        --target=*) ;;
        *) args="$args $a" ;;
    esac
done
exec /var/www/swirl/.toolchain/wasi-sdk-34.0-x86_64-linux/bin/clang --target=wasm32-wasip1 -mexception-handling -mllvm -wasm-enable-sjlj -mllvm -wasm-use-legacy-eh=true -DL_tmpnam=32 $args
