# Swirl plan

Swirl is the Rust rewrite of Chocolate (game client and server) and vanilla (software avatar renderer). This file is the working plan: mapping, order, parity method and live status. Update it with every milestone.

## Phases

1. vanilla to parity (avatars, headshots, animations, R15, items done; model, mesh and place scenes todo).
2. Switch Solar renders to Swirl behind one toggle file (`/var/www/html/core/swirl.enabled`) with PHP vanilla as the fallback. Code is in place and off by default; see Solar switch below.
3. 3D views with Swirl. Chosen approach: `swirl-vanilla` gets a `scene` mode that prints the data the viewer needs (per-part CFrames from `items::build_look`, converted meshes as typed arrays, textures as PNG URLs, body colours, stance), served next to the render endpoint. `js/mobile/avatar3d.js` asks for it and falls back to the current chocolate `avatarview.js` path on any error. Later, `swirl-render` compiled to wasm replaces the JS viewer. Not started.
4. swirl-core and swirl-server: instance tree, class tables, rbxl/rbxm readers, Lua host (mlua, lua51 + vendored), physics, humanoid, replication, WebSocket protocol identical to `chocolate/server`.
5. swirl-client (wasm32-unknown-unknown + wasm-bindgen, WebGL2 via wgpu or raw web-sys), 2012 and 2018 eras. The JS client keeps working against the Swirl server during the transition.

## vanilla mapping (PHP source to Rust)

| PHP | Rust | Status |
| --- | --- | --- |
| `BrickColors.php` | `swirl-core/brickcolors.rs` | done |
| `Geometry.php` | `swirl-core/math.rs` | done |
| `Rbxm.php` (XML) | `swirl-core/rbx.rs` | done |
| `Rbxm.php` (binary, LZ4, zstd) | `swirl-core/rbx.rs` | done; tree and property counts match PHP on 8 real binary models (only invalid-UTF-8 names differ) |
| `Img.php` | `swirl-render/img.rs` | PNG and JPEG; WebP, GIF, BMP todo (flagged unsupported); GD alpha quantisation matched; GD `imagescale` not matched exactly |
| `Texture.php` | `swirl-render/texture.rs` | done |
| `Meshes.php` | `swirl-render/mesh.rs` | v1 to v5 and uncompressed v6/v7 done; Draco todo (reported as unsupported so Solar falls back) |
| `LookBuilder.php` (clothes, face, hats, gear, welds, decals) | `swirl-render/look.rs`, `items.rs` | done except material tiles, `packageGripOf` |
| `Scene.php` draw (body, head views), `setup` | `swirl-render/scene.rs`, `raster.rs` | done |
| `ReferenceRaster.php` | `swirl-render/raster.rs` | done (bit exact vs PHP `exact`) |
| `FastRaster.php` | `swirl-render/fast.rs` | done for opaque geometry; glass (transparent items) and particles still use the reference rasteriser |
| `RigR15.php` | `swirl-render/r15.rs` | done (rig, pose, scale, stances, packages, accessories, tools) |
| `Character2016.php` | `swirl-render/char2016.rs` | todo (2016 renderer; only the Composit meshes it provides are used) |
| `ItemRender.php` | `swirl-render/body.rs`, `items.rs` | done: avatar base, item thumbnails (items, head, packages, thumbnail cameras) |
| `SceneRender.php` (places, sky, materials, terrain parts, eras) | `swirl-render/place.rs` | todo |
| `Particles*.php` | `swirl-render/particles.rs` | todo |
| `Vanilla.php`, `avatar.php` entry | `swirl-vanilla` | CLI done (ids, item, with, colors, size, view, rig, scale, pose, stance, straight-legs); exit 4 for unsupported formats, 5 for assets missing locally, 3 for item errors |
| `CharApp.php`, `Net.php`, `PngStream.php` | `swirl-vanilla` | todo (Arbiter passes ids and colours directly, so charapp fetching is low priority) |

## Chocolate mapping (JS to Rust)

| JS | Rust |
| --- | --- |
| `shared/instance.js`, `classes.js`, `classes2018.js`, `core2018.js`, `services.js`, `brickcolor.js`, `cframe.js` | `swirl-core` |
| `shared/rbxl.js`, `rbxlx.js`, `rbxlbin.js`, `codec.js` | `swirl-core` (readers and writers; shares `rbx.rs`) |
| `shared/luahost.js`, `api2018.js`, `runtime.lua`, `runtime2018.lua`, `debugger.js` | `swirl-lua` (mlua lua51 vendored; the .lua runtimes are reused unchanged) |
| `shared/physics.js`, `humanoid.js`, `animator.js`, `pathfinding.js`, `collisiongroups.js`, `csg.js`, `terrain*.js` | `swirl-physics` |
| `shared/teamsync.js`, `server/*.js` | `swirl-server` (tokio, tungstenite) |
| `client/*.js`, `client/ui/`, `studio/` | `swirl-client` (wasm); Studio last |
| `core/terrain-mesh.mjs` (Solar) | `swirl-physics` terrain mesher |
| `tools/` | stay JS or become cargo bins as needed |

Sizes for scoping: server/game.js 2.3k lines, shared/luahost.js 1.7k, client/render.js 1.7k, shared/physics.js 1.4k, client/world.js 1.4k, client/gui.js 1.3k, shared/humanoid.js 1.2k, the two Lua runtimes 2.7k, about 25k lines in client, server and shared, plus studio and tools.

What stays JS for now: the Studio UI, CoreScripts (Lua, run on the Lua host), content tooling, and the existing client until the wasm client reaches parity. HTML and CSS shells stay as they are.

## Port order

1. vanilla: R6 (done), hats and gear (done), R6 stances (done), CharacterMesh and head meshes, FastRaster, item and mesh renders, R15 and scale, packages, 2016 mode, place renders with era and terrain, binary rbxm, chunked meshes, JPEG.
2. swirl-core readers checked against real places and Chocolate's own dumps.
3. Lua host with the existing runtime.lua; physics; humanoid; server protocol; client.

## Parity method

`tools/parity/run.sh <name> <options>` renders with PHP vanilla (`tools/parity/php_render.php`: read-only fetcher over `/var/www/assets/assets` then `/cache`, throwaway cache dir under /tmp, never touches the live cache or slot locks) and with `swirl-vanilla`, then prints a pixel diff. `PARITY_EXACT=1` also renders PHP with `exact` (the reference rasteriser) and diffs against that, which isolates port errors from the fast-rasteriser approximation. Outputs are in `/tmp/swirl/<name>.{php,phpexact,swirl,diff,diffexact}.png`.

## Solar switch

`core/class/Arbiter.php` (`swirlRender`, used by avatar, headshot, animation, R15 package and item renders) and `core/vanilla-rcc/avatar.php` (`swirl_png`, `/api/v1/render` for 2012, no zoom/yaw/pitch, png, smooth) run `/var/www/swirl/target/release/swirl-vanilla` as a child process with an argument list (no shell), a 45 second limit and a temp PNG under /tmp. Any non-zero exit, timeout or empty image falls back to the PHP renderer, and Arbiter logs `swirl fallback code=N` to render.log. Exit codes: 3 item error, 4 unsupported asset format, 5 asset not on local disk (Solar's PHP path then fetches it over HTTP as before). The switch is the file `/var/www/html/core/swirl.enabled`: create it to turn Swirl on, delete it to turn it off at once. It does not exist yet. `RENDER_VERSION` in avatar.php was bumped to `vanilla-1.7-lighting-12`. Model, mesh and place renders (`renderScene`) always use PHP until ported.

## Parity status

Mean absolute error per channel (rgba), percent of pixels off by more than 8, max difference, against PHP in its default mode (the fast rasteriser, what Solar runs). Test assets come from /var/www/assets; PNGs in /tmp/swirl/p_*.

| Case | mae / pct>8 / max |
| --- | --- |
| plain R6 352x352, head view 150x150 | 0 / 0% / 0 |
| shirt + pants + hat + gear | 0.066 / 0% / 8 |
| face decal head view, hats 102+112, accessories 410+539, gear 1011, head meshes 1086 | 0 / 0% / 0 |
| stances walking (hat+gear) | 0.065 / 0.003% / 16; sitting 0 |
| CharacterMesh package 1106; 1302+1304+1306+hat | 0.0001 / 0% / 2; 0.082 / 0.006% / 16 |
| R15 plain, pose 507766388; with gear | 0 / 0% / 0 |
| R15 scaled with hats; R15 package 1106 + hat; R15 head view | 0.04 / 0.001% / 16; 0.076 / 0.002% / 16; 0.105 / 0% / 6 |
| item thumbnails 512x512: accessory 410, gear 1011, shirt 1287, face 121, head 1086, package 1106 | 0 / 0% / 0-3 |
| item thumbnail: hat 1005 | 0.42 / 0.16% / 17 |

Exact mode (PHP `exact => true`) also matches pixel for pixel on all avatar cases. The only recorded residuals come from textures whose sides are not powers of two: PHP scales them with GD `imagescale` and Swirl with its own bilinear. Speed: 0.2 s per body render against 1.1 s for PHP.

Not yet at parity or not ported: glass (transparent items) and particles in the fast rasteriser, model/mesh/place scenes, eras and terrain parts, 2016 renderer, Draco meshes, WebP/GIF textures, zoom/yaw/pitch camera options, jpg/webp output, the PHP default for assets fetched over HTTP.
