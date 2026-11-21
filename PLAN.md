# Swirl plan

Swirl is the Rust rewrite of Chocolate (game client and server) and vanilla (software avatar renderer). This file is the working plan: mapping, order, parity method and live status. Update it with every milestone.

## Phases

1. vanilla to parity (in progress, see status below).
2. Switch Solar renders to Swirl behind one toggle file with PHP vanilla as the fallback. Not started: it needs the full parity table first.
3. 3D views with Swirl. Chosen approach: `swirl-vanilla` gets a `scene` mode that prints the data the viewer needs (per-part CFrames from `items::build_look`, converted meshes as typed arrays, textures as PNG URLs, body colours, stance), served next to the render endpoint. `js/mobile/avatar3d.js` asks for it and falls back to the current chocolate `avatarview.js` path on any error. Later, `swirl-render` compiled to wasm replaces the JS viewer. Not started.
4. swirl-core and swirl-server: instance tree, class tables, rbxl/rbxm readers, Lua host (mlua, lua51 + vendored), physics, humanoid, replication, WebSocket protocol identical to `chocolate/server`.
5. swirl-client (wasm32-unknown-unknown + wasm-bindgen, WebGL2 via wgpu or raw web-sys), 2012 and 2018 eras. The JS client keeps working against the Swirl server during the transition.

## vanilla mapping (PHP source to Rust)

| PHP | Rust | Status |
| --- | --- | --- |
| `BrickColors.php` | `swirl-core/brickcolors.rs` | done |
| `Geometry.php` | `swirl-core/math.rs` | done |
| `Rbxm.php` (XML) | `swirl-core/rbx.rs` | done |
| `Rbxm.php` (binary, LZ4) | `swirl-core/rbx.rs` | todo |
| `Img.php` | `swirl-render/img.rs` | PNG only; JPEG, WebP, GIF todo; GD alpha quantisation matched |
| `Texture.php` | `swirl-render/texture.rs` | done |
| `Meshes.php` | `swirl-render/mesh.rs` | v1 to v5 done; v6/v7 chunked and `Draco.php` todo |
| `LookBuilder.php` (clothes, face, hats, gear, welds, decals) | `swirl-render/look.rs`, `items.rs` | done except CharacterMesh, head SpecialMesh, material tiles |
| `Scene.php` draw (body, head views), `setup` | `swirl-render/scene.rs`, `raster.rs` | done |
| `ReferenceRaster.php` | `swirl-render/raster.rs` | done (bit exact vs PHP `exact`) |
| `FastRaster.php` | `swirl-render/raster.rs` | todo (PHP default; differs from reference by about 0.1 to 0.5 mean error) |
| `RigR15.php`, `Character2016.php` | `swirl-render/r15.rs`, `char2016.rs` | todo (R6 stances done in `items.rs`) |
| `ItemRender.php` (item and package thumbnails, `characterBase2012`) | `swirl-render/item.rs` | todo |
| `SceneRender.php` (places, sky, materials, terrain parts, eras) | `swirl-render/place.rs` | todo |
| `Particles*.php` | `swirl-render/particles.rs` | todo |
| `Vanilla.php`, `avatar.php` entry | `swirl-vanilla` | CLI done (ids, colors, size, view, stance); HTTP and request validation todo |
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

## Parity status

Numbers are mean absolute error per channel (rgba), percentage of pixels off by more than 8, and max difference. "exact" is against PHP `exact => true`; "fast" is against the PHP default.

| Case | exact | fast |
| --- | --- | --- |
| plain R6, grey, 352x352 | 0 / 0% / 0 | 0.06 / 0.08% / 23 |
| head view, 150x150 | 0 / 0% / 0 | 0.23 / 0.55% / 22 |
| shirt + pants, 420x420 | 0 / 0% / 0 | 0.37 / 1.0% / 176 |
| face decal, head view | 0 / 0% / 0 | not recorded |
| hat 1005 (non-power-of-two texture) | 0.08 / 0.008% / 16 | 0.4 / 1.4% / 32 |
| hats 102+112 | 0 / 0% / 0 | 0.43 / 2.2% / 48 |
| accessory 410+539 | 0 / 0% / 0 | 0.36 / 2.4% / 255 |
| gear 1011 | 0 / 0% / 0 | 0.26 / 1.1% / 39 |
| gear 1049 + hat 1002 | 0 / 0% / 0 | 0.24 / 0.9% / 38 |
| head view with hats | 0.07 / 0% / 8 | 0.79 / 3.0% / 28 |
| stances walking, sitting, overlord with hat and gear | 0.07 / 0.002 to 0.005% / 16 | not recorded |

The only difference against exact comes from resizing non-power-of-two textures (PHP uses GD `imagescale`, Swirl its own bilinear). Speed: about 0.2 s per body render against 1.1 s for PHP.

Known gaps (each needs a PHP case before it counts as done): CharacterMesh and head meshes, R15 and scale, packages, item/mesh/model/animation thumbnails, place scenes, eras, terrain, 2016 renderer, particles, material tiles, binary models, chunked meshes, JPEG textures, FastRaster.
