# Swirl plan

Swirl is the Rust rewrite of Chocolate (game client and server) and vanilla (software avatar renderer). This file is the working plan: mapping, order, parity method and live status. Update it with every milestone.

## Phases

1. vanilla to parity: done for avatars, headshots, animations, R15, items, model, mesh and place scenes (with era and terrain parts). Remaining: Draco meshes, WebP/GIF textures, 2016 renderer, scene Hint/Message overlays, jpg/webp output; these exit 4 and fall back to PHP.
2. Switch Solar renders to Swirl behind one toggle file (`/var/www/html/core/swirl.enabled`) with PHP vanilla as the fallback. Code is in place and off by default; see Solar switch below.
3. 3D views. Finding: the site's 3D views (`avatar3d.js`, `chocolate/client/avatarview.js`) never call vanilla or Arbiter; they build the character in the browser from `/asset` data with chocolate's own renderer, so they keep working unchanged while renders move to Swirl. The Swirl-driven viewer is part of phase 5 (swirl-client compiled to wasm). Original idea, kept for phase 5: `swirl-vanilla` gets a `scene` mode that prints the data the viewer needs (per-part CFrames from `items::build_look`, converted meshes as typed arrays, textures as PNG URLs, body colours, stance), served next to the render endpoint. `js/mobile/avatar3d.js` asks for it and falls back to the current chocolate `avatarview.js` path on any error. Later, `swirl-render` compiled to wasm replaces the JS viewer. Not started.
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

`core/class/Arbiter.php` (`swirlRender`, used by avatar, headshot, animation, R15 package and item renders) and `core/vanilla-rcc/avatar.php` (`swirl_png`, `/api/v1/render` for 2012, no zoom/yaw/pitch, png, smooth) run `/var/www/swirl/target/release/swirl-vanilla` as a child process with an argument list (no shell), a 45 second limit and a temp PNG under /tmp. Any non-zero exit, timeout or empty image falls back to the PHP renderer, and Arbiter logs `swirl fallback code=N` to render.log. Exit codes: 3 item error, 4 unsupported asset format, 5 asset not on local disk (Solar's PHP path then fetches it over HTTP as before). The switch is the file `/var/www/html/core/swirl.enabled`: create it to turn Swirl on, delete it to turn it off at once. It does not exist yet. `RENDER_VERSION` in avatar.php was bumped to `vanilla-1.7-lighting-12`. Model, mesh and place renders go through `swirlScene` in Arbiter (place bytes and the terrain mesh blobs from `withTerrainParts` are written to /tmp and passed as `--scene-file` and `--content-dir`), with the same fallback. `tools/install.sh` also gates on a model, a place and a mesh parity check. The binary rejects unknown options (exit 2) so a stale binary can never answer a newer request wrongly.

## Parity status

Mean absolute error per channel (rgba), percent of pixels off by more than 8, max difference, against PHP in its default mode. PNGs in /tmp/swirl.

| Case | mae / pct>8 / max |
| --- | --- |
| plain R6, head view, face, hats, accessories, gear, head meshes, R15 plain and with gear | 0 / 0% / 0 |
| shirt, pants, hat, gear; stances | 0.065 / 0% / 8 to 16 |
| CharacterMesh packages; R15 packages, scale, head view | 0 to 0.1 / 0% / 2 to 16 |
| item thumbnails (accessory, gear, shirt, face, head, package) | 0 / 0% / 0 to 3 |
| item thumbnail hat 1005, zoom/yaw/pitch variants | 0.1 to 0.55 / up to 0.24% / 15 to 18 |
| model thumbnails 512x512 (assets 11, 1085, 1074, 1, 1002, 1005, 1011, 1015, 1031, 104, 1043) | 0.01 to 0.18 / 0% / 3 |
| mesh thumbnail (asset 1000) | 0.001 / 0% / 3 |
| place thumbnails 854x480, era 2012 (assets 1939, 352, 564, 1197) | 0.29 to 0.42 / 0% / 1 to 3 |
| place thumbnail, era 2018 (asset 3) | 0.54 / 0% / 4 |
| place with Chocolate terrain (asset 1968 via withTerrainParts), 2012 and 2018 | 0.42, 0.53 / 0% / 3, 5 |

Residuals: textures with sides that are not powers of two (PHP uses GD `imagescale`), skybox decoding and scaling (JPEG decoder and GD scaling differ slightly), and the final GD `imagecopyresampled` (ported in f32 from memory, matches to within 3 levels). Speed: 0.2 s per body render, 0.3 to 8 s per place against PHP's several seconds.

## Chocolate port status

- swirl-core: class table (394 classes, 183 enums) dumped from `chocolate/shared/classes.js` by `tools/dump-classes.mjs` into `data/classes.json`, resolved in `classes.rs` (is_a, canonical_prop, creatable, services, eras); `instance.rs` has the DataModel arena (create, services, parent with cycle check, props with defaults, find, descendants, clone, destroy, load from an rbx tree). Tests pass, including loading a real place.
- swirl-lua: mlua with lua51 and vendored builds and reports Lua 5.1 semantics (no string.split, setfenv and unpack present).
- Not started: Lua host bindings (luahost.js, runtime.lua), signals and replication, physics, humanoid, server protocol, wasm client.

## Server and client slice (1.0)

- `swirl-server` (tokio, tokio-tungstenite): `--place=ID --port=N` (refuses 4201 and 4202) loads the place, creates Chocolate's service set, a Camera and Terrain, and answers each WebSocket join with Chocolate's `init` message (records `[id, class, parent, props]` with default and hidden properties dropped, Color derived from BrickColor, scoped player containers), `sys`, and `pong` for `ping`. No ticket check, no scripts, no characters, no physics, no `rep` flushes yet. `tools/cmp-init.py` compares its snapshot with the JS server's (`tools/js-init.mjs`) for a place: places 1939, 352, 564 and 1197 differ only in time-dependent and team properties.
- `swirl-net`: client-side world from `init` and `rep` ops (p, m, n, d) with native tests. `swirl-client` 1.0: wasm-bindgen glue and a WebGL2 box renderer with an orbit camera; published as developer release 1.0. Verified headless: the wasm module parses Chocolate's own `init` snapshots and a Swirl server join in node (7 parts for place 564). The WebGL drawing path has not been run in a browser.
- Next: Lua host (luahost.js and runtime.lua semantics), signals and `rep` flushing, character spawn and humanoid, physics, then client features (meshes, textures, GUI, input) toward the 2012 and 2018 clients.
