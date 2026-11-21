---
name: build-client
description: Build, time and release the Swirl wasm client. Use when asked to build, rebuild, speed up or publish the Swirl client, or to check build times.
---

# Building the Swirl client

- `tools/build-client.sh [outdir]` builds `swirl-client` for wasm32 with the `client` profile (opt-level 2, no LTO, 16 codegen units, incremental, panic abort), runs `wasm-bindgen --target web` and writes `entry.js` and `manifest.json` ({version "N.M", era, entry, build hash}) next to the wasm. Default output `/var/www/swirl/dist/client`. `PROFILE=client-size` gives a smaller thin-LTO build; `WASM_OPT=1` runs `wasm-opt -O1` when it is installed.
- `tools/release-client.sh` builds and runs `php /var/www/html/tools/release.php publish developer <N.M> <dir> --entry=entry.js`; set `CURRENT=1` to make it the current developer release and `NOTES="..."` for the notes. Releases are N.M (1.0, 1.1, ...) taken from the crate version; bump `crates/swirl-client/Cargo.toml` (and the other crates) for a new minor.
- The entry runs as soon as it is imported when `window.solarClientConfig` exists, uses `#swirl-canvas` and `#swirl-status`, requests the ticket like Chocolate's client, joins `<ws>?ticket=`, calls `window.solarClientReady()` when the snapshot is drawn and `window.solarClientFailed(reason)` on errors.
- Tests without a browser: `wasm-bindgen --target nodejs --out-dir /tmp/swirl/wnode target/wasm32-unknown-unknown/client/swirl_client.wasm`, then `node tools/wasm-load-test.cjs /tmp/swirl/wnode <init.json>` or start `swirl-server --place=ID --port=<spare>` and run `node tools/wasm-join-test.cjs /tmp/swirl/wnode <port>`. WebGL drawing cannot be checked on the server.
- Keep the client crate small: protocol and world logic live in `swirl-net` (native tests), GL glue in `swirl-client`. Debug builds: workspace `dev` profile is opt-level 1 with dependencies at opt-level 2; rust's own lld linker is the default on this toolchain.

Measured on this box (4 cores): clean wasm build 27 s (client profile), 34 s for `client-size`; one-line edit in swirl-client or swirl-net 0.9 s, in swirl-core 1.8 s, no-op 0.3 s (cargo plus wasm-bindgen).
