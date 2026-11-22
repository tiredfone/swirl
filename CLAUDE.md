# Swirl

Rust rewrite of Chocolate (browser game client and node game server) and vanilla (PHP software avatar renderer), used by Solar (/var/www/html). Codename Swirl. Multi-session project; PLAN.md holds the module mapping, port order and parity status. Keep it current.

## Layout

| Crate | Replaces |
| --- | --- |
| `crates/swirl-core` | math, instance tree, classes, rbxm/rbxl binary and XML readers, mesh readers (Chocolate `shared/`, vanilla `Rbxm.php`, Solar `RobloxFile`) |
| `crates/swirl-render` | vanilla `src/` software renderer (later shared with the wgpu/WebGL client) |
| `crates/swirl-vanilla` | vanilla `avatar.php` entry: CLI and HTTP with the same inputs and PNG output |
| `crates/swirl-physics` | Chocolate physics and humanoid |
| `crates/swirl-lua` | Lua 5.1 host through mlua (lua51, vendored) |
| `crates/swirl-server` | Chocolate `server/`, same WebSocket protocol |
| `crates/swirl-client` | Chocolate `client/`, wasm |
| `crates/swirl-bootstrapper` | Solar desktop launchers `SolarPlayer2012Launcher` and `SolarPlayer2018Launcher` (install, auto-update, protocol, window) |
| `crates/swirl-desktop` | native client `SolarPlayer` (winit, glutin, glow, tungstenite) over `swirl-net` |
| `tools/parity/` | PHP side of the parity harness and the pixel diff runner |

## Commands

- Rust lives in `~/.cargo/bin`; run `export PATH=$HOME/.cargo/bin:$PATH`.
- `cargo build --release`, `cargo test`.
- Parity: see the `render-parity` skill. Outputs go to `/tmp/swirl`.

## Rules

- No comments in any code (Rust, PHP, JS, shell). Doc files (PLAN.md, README.md, CLAUDE.md, skills) are fine.
- Never print secrets. Never read `/var/www/html/core/config.php` or `chocolate/config.local.php`.
- Do not modify anything under `/var/www/html` or `/var/www/assets`, and do not touch the live servers (ports 4201 and 4202) or the database. Read them only.
- PHP vanilla runs in the harness with a throwaway cache dir under /tmp and a read-only fetcher over `/var/www/assets/assets` then `/var/www/assets/cache`.
- Use f64 where the PHP uses floats so results match; keep the PHP algorithm order unless a change is recorded in PLAN.md.
- Stage files by name; commit locally; push with `git push -u origin main` (never force). If the remote repo does not exist yet, keep committing and tell the owner.

## Status

See PLAN.md. Everything Solar renders is ported and Solar uses only Swirl (no PHP fallback, no toggle file): avatars (R6 and R15, scale, pose, stances, packages), headshots, animation, item, model, mesh and place thumbnails (era, terrain parts, Hint and Message overlays), Draco meshes and GIF, WebP and BMP textures. The 2016 renderer is not ported (Solar never selects it). The binary is `bin/swirl` and Solar calls it through `core/class/Swirl.php`, with render data in `/var/www/assets/render-data` (`--data`, never in this repository). Subcommands: `mesh-convert <in> <out>` and `mesh-json <in> <out>` (Solar's mesh converter, `version 1.00` output, byte-identical to the old PHP), `body-colors <file>` and `character-colors <file>` (six `#rrggbb` colours), `image-png <in> <out>`, `diff`, `dump`. Options `--extra-assets=DIR`, `--allow-missing` (exit 0 despite `missing: <id>` lines on stderr) and `--smooth=0`. Exit codes: 2 unknown option, 3 item error, 4 unsupported format, 5 asset not on local disk.

## Installing for Solar

Solar runs `/var/www/swirl/bin/swirl`, never the build dir. `tools/install.sh` builds `swirl-vanilla` (the package that produces the `swirl` binary), runs the parity gates against a frozen PHP copy in `tools/parity/php-ref` (untracked, git-ignored; its `data` is a symlink to `/var/www/assets/render-data`): five avatar and item checks (every channel under 0.5 mean error), model, place, mesh and place-overlay scene checks, WebP, GIF and BMP decode checks (exact), and `mesh-convert` against the sha256 hashes in `tools/parity/mesh-golden.txt`; only then it copies the binary to `bin/` through a temp file and `mv`. Run it after every renderer change that should go live; `bin/` is not in git.

## Client builds

The wasm client (`swirl-client`, glue over `swirl-net`) is built with `tools/build-client.sh` and released with `tools/release-client.sh` to Solar's developer channel (versions 1.0, 1.1, ...). Timings: clean 27 s, incremental 1 to 3 s. Chocolate is vendored in `vendor/chocolate/` (client, shared, content, content2018 without `platform`, `corescripts2018/bundle.json`; refresh with `tools/sync-chocolate.sh`, drift report `tools/check-chocolate-drift.sh`, run by the build). The build copies it (hardlinks) to `<out>/chocolate`, and `lua.js` (beside `entry.js`) loads Chocolate's engine from there: real world, rapier physics, Controller, camera, Animate scripts, GUI. Nothing may request `/chocolate/` (the release gate fails on it). The Rust client is the renderer only: `client.set_driven(true)` makes it take camera and CFrames from JS. See PLAN.md, Client Lua and GUI. See the `build-client` skill. `wasm-bindgen` 0.2.100 is installed in `~/.cargo/bin` and the crate is pinned to the same version.

## Renderer parity

The wasm renderer (`crates/swirl-client/src/gl.rs`) ports Chocolate's shaders (shadows, studs, materials, decals, limb atlases, sky cube, terrain chunks); see PLAN.md, Renderer parity. Compare with `tools/compare-shots.cjs` (paired shots in `/tmp/swirl/compare`) and `tools/pair-diff.sh`. `tools/sync-chocolate.sh` also copies the used `content2018/platform` textures.

## Desktop app

The launchers and the native client are built with `tools/release-desktop.sh` (profiles `launcher` and `client`) and published to Solar's developer channel with `release.php publish-desktop`. See the `bootstrapper` and `desktop-release` skills and `docs/bootstrapper-research.md`. Linux builds run here; Windows needs mingw-w64, which is not installed.

## Port order

1. vanilla: R6 avatar, then hats, gear, R15, items, places.
2. swirl-core readers.
3. Lua host, physics, server, client.

## Server and client tests

- `swirl-server --port=N` is a supervisor like Chocolate's `main.js`: it picks or starts a place instance (one thread each) from the redeemed ticket. It reads `config.local.php` (gitignored; template `config.example.php`; keys `redeem_url`, `redeem_key`, `site_url`, optional `port`, `assets_dir`, `data_dir`, `presence_path`, `log_file`) and CLI flags override it (`--config=`, `--assets=`, `--store=`, `--site=`, `--data=`, `--redeem-url=`, `--redeem-key-file=`, `--no-redeem`, and `--place=ID --era=N` for the default place when tickets are not checked). Never print or log the key. Refuses ports 4201 and 4202. It serves `/ws`, `/status`, and the keyed POSTs `/kick`, `/shutdown`, `/run`, `/restart`.
- Swirl does not depend on Chocolate: Lua runtimes and tables are in `crates/swirl-lua/data/`, character scripts and the 2018 core bundle in `data/server/`, stores in `data/store/` (gitignored). `tools/compare-chocolate.sh` only reports drift. `tools/dump-events.mjs`, `dump-members.mjs`, `dump-stubs.mjs`, `dump-luadata.mjs` regenerate the tables from Chocolate on purpose; they are not part of the build.
- Stop test servers with `pkill -x swirl-server` (not `pkill -f`). Tests: `tools/parity-server/` (see the `server-parity` skill) compares script output with Chocolate's own `Game`; `tools/swirl-ticket-join.cjs <port> <place> [secs]` gets a real ticket for the test account (one at a time) and joins; `tools/swirl-wasm-join.cjs <place> <client version> <port> [secs]` loads the wasm client with `?swirlws=` against a `--no-redeem` server (the dev override sends the ticket `dev`). Older node tests: `tools/swirl-tool-test.mjs`, `swirl-remote-test.mjs`, `swirl-touch-test.mjs`, `swirl-char-test.mjs`, `swirl-local-test.cjs`; they need `--no-redeem`. Place 1669 has the classic tools in StarterPack. Server feature status and what is missing: PLAN.md, "Server port".

## Skills

- `port-module`: port a Chocolate or vanilla file to Rust and check parity.
- `server-parity`: compare swirl-server with Chocolate's game server on a place and read the diff.
- `render-parity`: run and read the vanilla pixel diff.
- `subagent-tasks`: hand work to Sonnet subagents.
- `bootstrapper`: the desktop launchers and native client, and how to test them.
- `desktop-release`: build and publish the desktop app.
