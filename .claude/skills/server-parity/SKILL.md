---
name: server-parity
description: Compare swirl-server with Chocolate's game server on a place and find missing server features. Use when porting server code (Lua host methods, humanoid, physics, services) or checking that a game behaves like Chocolate.
---

# Server parity

1. Build: `export PATH=$HOME/.cargo/bin:$PATH; cargo build --release -p swirl-server`.
2. Chocolate side: `cd /var/www/swirl/tools/parity-server && CHOCOLATE_DATA_DIR=/tmp/claude-1000/chocdata node choc-run.mjs <place> <2012|2018> <ms>` runs Chocolate's real `Game` headless (it loads Chocolate's config through PHP; nothing is printed, social calls use a dummy key and fail) and writes `/tmp/swirl/par/<place>.choc.log`.
3. Swirl side: `./swirl-run.sh <place> <era> <spare port> <ms>` starts `swirl-server --no-redeem`, joins once and writes `/tmp/swirl/par/<place>.swirl.raw`.
4. `python3 compare.py <place>` prints lines (error, warn, print, with script path, line number and player name removed) that differ in count. Fix the Swirl side until it prints `identical script output`.
5. A missing method shows as `X is not a valid member of Y`. Find Chocolate's `def('X', 'Y', ...)` in `shared/luahost.js` or `api2018.js`, and implement it in `crates/swirl-server/src/api.rs` (the `ApiHandler`: `call` gets `&mut DataModel`, the instance id, the method and `Wire` args; return `Res::Ok`, `Res::Async(token)` resolved later through `Host::resolve`, or `Res::Err`) or, for plain DataModel methods, in `host.rs`. Add the name to `api::METHODS`. Constant-returning stubs come from `data/stubs.json`.
6. Real tickets: `tools/swirl-ticket-join.cjs <port> <place>` with a server started without `--no-redeem` (reads `config.local.php`). `curl http://127.0.0.1:<port>/status` shows instances; keyed endpoints need `X-Chocolate-Key` (read the key from `config.local.php` into a shell variable, never print it).
7. Browser: `tools/swirl-wasm-join.cjs <place> <version> <port> 30` against a `--no-redeem` server; `SHOT=file.png` saves a screenshot.
8. Record results in PLAN.md (the feature table). Stop only the servers you started, by PID (`kill $SP`); never `pkill -x swirl-server`, a permanent server runs on port 4200.
