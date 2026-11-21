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

See PLAN.md. Avatars (R6 and R15, scale, pose, stances, packages), headshots, animation thumbnails and item thumbnails are ported and match PHP's default renderer (see the parity table). Model, mesh and place scenes are not ported and stay on PHP. Solar calls the `swirl-vanilla` binary through `core/class/Arbiter.php` and `core/vanilla-rcc/avatar.php` only while `/var/www/html/core/swirl.enabled` exists, and falls back to PHP on any error. Exit codes: 3 item error, 4 unsupported format, 5 asset not on local disk.

## Installing for Solar

Solar runs `/var/www/swirl/bin/swirl-vanilla`, never the build dir. `tools/install.sh` builds, runs five parity checks against PHP (every channel must stay under 0.5 mean error) and only then copies the binary to `bin/` through a temp file and `mv`. Run it after every renderer change that should go live; `bin/` is not in git.

## Port order

1. vanilla: R6 avatar, then hats, gear, R15, items, places.
2. swirl-core readers.
3. Lua host, physics, server, client.

## Skills

- `port-module`: port a Chocolate or vanilla file to Rust and check parity.
- `render-parity`: run and read the vanilla pixel diff.
- `subagent-tasks`: hand work to Sonnet subagents.
