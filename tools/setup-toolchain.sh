#!/bin/sh
set -e
cd /var/www/swirl
tmp=$(mktemp -d)
curl -sL -o "$tmp/wasi-sdk.tar.gz" https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34/wasi-sdk-34.0-x86_64-linux.tar.gz
mkdir -p .toolchain
tar xzf "$tmp/wasi-sdk.tar.gz" -C .toolchain
rm -rf "$tmp"
