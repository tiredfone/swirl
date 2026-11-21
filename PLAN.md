# Swirl plan

Swirl is the Rust rewrite of Chocolate (game client and server) and vanilla (software avatar renderer). This file is the working plan: mapping, order, parity method and live status. Update it with every milestone.

## Phases

1. vanilla to parity: done for avatars, headshots, animations, R15, items, model, mesh and place scenes (with era and terrain parts). Draco meshes, WebP/GIF/BMP textures, scene Hint/Message overlays and a `--smooth=0` option are done; jpg/webp output is done in Solar's endpoint with GD; the 2016 renderer is dropped (Solar never selects it).
2. Switch Solar renders to Swirl: done. Solar calls only `bin/swirl`; the PHP renderer and the toggle file are gone; see Solar switch below.
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
| `Meshes.php` | `swirl-render/mesh.rs` | v1 to v5 and uncompressed v6/v7 done; Draco done (`swirl-core/draco.rs`, shared with `meshconv.rs`) |
| `LookBuilder.php` (clothes, face, hats, gear, welds, decals) | `swirl-render/look.rs`, `items.rs` | done except material tiles, `packageGripOf` |
| `Scene.php` draw (body, head views), `setup` | `swirl-render/scene.rs`, `raster.rs` | done |
| `ReferenceRaster.php` | `swirl-render/raster.rs` | done (bit exact vs PHP `exact`) |
| `FastRaster.php` | `swirl-render/fast.rs` | done for opaque geometry; glass (transparent items) and particles still use the reference rasteriser |
| `RigR15.php` | `swirl-render/r15.rs` | done (rig, pose, scale, stances, packages, accessories, tools) |
| `Character2016.php` | `swirl-render/char2016.rs` | todo (2016 renderer; only the Composit meshes it provides are used) |
| `ItemRender.php` | `swirl-render/body.rs`, `items.rs` | done: avatar base, item thumbnails (items, head, packages, thumbnail cameras) |
| `SceneRender.php` (places, sky, materials, terrain parts, eras) | `swirl-render/place.rs` | todo |
| `Particles*.php` | `swirl-render/particles.rs` | todo |
| `Vanilla.php`, `avatar.php` entry | `swirl-vanilla` (binary `swirl`) | CLI done (ids, item, with, colors, size, view, rig, scale, pose, stance, straight-legs); exit 4 for unsupported formats, 5 for assets missing locally, 3 for item errors |
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

`tools/parity/run.sh <name> <options>` renders with the frozen PHP reference in `tools/parity/php-ref` (`tools/parity/php_render.php`: read-only fetcher over `/var/www/assets/assets` then `/cache`, throwaway cache dir under /tmp, never touches the live cache or slot locks) and with `swirl-vanilla`, then prints a pixel diff. `PARITY_EXACT=1` also renders PHP with `exact` (the reference rasteriser) and diffs against that, which isolates port errors from the fast-rasteriser approximation. Outputs are in `/tmp/swirl/<name>.{php,phpexact,swirl,diff,diffexact}.png`.

## Solar switch

Done. `core/class/Swirl.php` in Solar runs `/var/www/swirl/bin/swirl` with an argument list (no shell, 45 second limit), `--data=/var/www/assets/render-data` and `--assets=SOLAR_ASSETS`. `Arbiter` (avatar, headshot, animation, R15 package, item, model, mesh and place renders) calls only Swirl; on a failure it keeps the old image and logs to render.log. Assets that are on neither `assets/` nor `cache/` are printed as `missing: <id>` on stderr; `Solar\Swirl::render` fetches them from `/asset` into a temporary directory passed as `--extra-assets` and runs again, at most twice. `/api/v1/render` is `router/api/render/v1.php` (2012 renderer only; png, jpg and webp through GD). `MeshConverter` uses `mesh-convert` and `mesh-json`, and the body-colour and character-colour helpers use `body-colors` and `character-colors`. Render cache busting is `SOLAR_RENDER_VERSION` in `core/render.php`. The binary rejects unknown options (exit 2) so a stale binary can never answer a newer request wrongly.

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

## Client 1.2 (developer channel)

Walking works against the live Chocolate server: `swirl-net::controller` (capsule of three spheres against oriented boxes, gravity 196.2, WalkSpeed and JumpPower from the Humanoid, jump only when grounded, yaw faces the move direction), WASD or arrows, space, a follow camera (drag to rotate, wheel to zoom), `char` messages at 30 Hz, and the server side messages `after.own` and `after.tp`, `ph` and `ch` applied to the world. Characters move as rigid models (no limb animation yet). Spheres and cylinders are drawn. The release gate now also walks and jumps (`WALK=1 tools/browser-check.cjs`, moved at least 3 studs and rose at least 1).

Still missing in the client: meshes, textures and decals, sky and lighting, limb animation and Humanoid states, tools and mouse, chat and GUI (CoreScripts need the Lua host), terrain, 2018 rigs. Still missing in the server: Lua host, rep flushing, character loading, physics.

## Client 1.4

File meshes (SpecialMesh FileMesh and MeshPart) and their textures are fetched by `entry.js` from the asset endpoint on request (`take_requests`, `provide_mesh`, `provide_texture`, `provide_failed`), parsed by `swirl-core::meshfile` (shared with the renderer) and `swirl-net::meshdata`, and drawn with a textured shader. Wedge parts, SpecialMesh wedge, sphere and cylinder types, and mesh Scale on primitives are drawn. Still missing: Decal and Texture instances, head meshes (content files), Sky and Lighting properties, shirts and pants on characters, limb animation.
Note: the live server kicks the test account if it reconnects too quickly ("started too many ..."); the release gate may need a minute between runs.

## Desktop app (launchers and native client)

- `swirl-bootstrapper` 1.0: two launchers from one library (2012 and 2018 looks, own wording, Solar logo). Flow: update check, launcher self-update (download to `launcher-<ver>/`, re-register, re-exec with the same args), client install (sha256 while streaming, zip into `Versions/<ver>`, copy of the launcher cloned beside it, protocol and `.desktop`/Start Menu registration, keep two newest versions), then start `SolarPlayer` and close when it prints `SOLAR_READY`. Errors show Retry/Close. Tested on Linux against a local fake server: fresh install, launch with arguments, updates 1.0 to 1.3 with cleanup, checksum mismatch, self-update with re-exec, and the real window under Xvfb.
- `swirl-desktop` 1.0: native `SolarPlayer`: redeem (`POST /api/v1/client/launch/redeem`), join over wss (tungstenite + rustls), `swirl-net` World and Controller, glow renderer with the wasm client's box shaders. Verified through `--offscreen` (EGL pbuffer under Xvfb) against swirl-server and against a real launch token for place 564.
- Not verified: the winit window path (this server lacks `libxkbcommon-x11`), anything on Windows (no mingw linker; `cargo check` for the Windows target passes with a dummy C compiler), the protocol handoff from a browser.
- Next: move the game loop (`swirl-desktop/src/game.rs`) and the shape meshes (`shapes.rs`) into `swirl-net` so the wasm client and the desktop client share them, then follow the wasm client's features (meshes, textures, sky, GUI). Code signing and a signed manifest for launcher self-update. Windows build and test. A site download page for the launchers.

## Client 1.10 and 1.11

1.10: Sky box from the Sky instance (or Chocolate's default `content/sky` faces) drawn per pixel with vanilla's face mapping, scene lighting from the Lighting instance (vanilla's `lightingFrom`), and Decal and Texture instances drawn on part faces. 1.11: procedural limb swing for R6 characters (own and others, from speed and state; arms up when airborne). The limb angles are not sent to the server because the Motor angle convention of Chocolate's `j` field is not matched yet.
Release rules: only real versions are published; the release script runs the walk gate on place 564 and the mesh gate on place 972, waiting 80 s between joins.

## Lua host and rep flushing (server slice 2)

- `swirl-lua::Host` loads Chocolate's `runtime.lua` unchanged (copy in `crates/swirl-lua/data/`, plus `luadata.lua` generated by `tools/dump-luadata.mjs`) into Lua 5.1 through mlua (`unsafe_new`, because the runtime needs the debug library and removes it itself). It implements the host table the runtime expects: `get`, `set`, `call`, `new`, `sub`, `unsub`, `poll`, `alive`, `running`, `log`, `now`, `side`, `data`, with the tagged flat wire format.
  Supported so far: properties by class table (bool, string, number, enum, V3, C3, CF, Ref, BC), Parent, Name, ClassName, children and services by name, methods GetChildren, GetDescendants, FindFirstChild (recursive flag), FindFirstChildOfClass, FindFirstChildWhichIsA, GetFullName, IsA, IsDescendantOf, IsAncestorOf, Clone, Destroy, ClearAllChildren, GetService, Instance.new, and the events ChildAdded, ChildRemoved, DescendantAdded, Changed (property sets from Lua), PlayerAdded.
  Not yet: UDim2/Vector2/Ray/enum wire types in set, Touched and every physics or humanoid event, Humanoid and Player methods, RunService events, BindableEvent and RemoteEvent, LoadAsset and the other async methods, CoreScripts, `rep` of changes made by code other than the Lua host.
- `swirl-core::DataModel` keeps a change journal; `swirl-server` flushes it every 33 ms as Chocolate `rep` operations (`p`, `n` with whole subtrees, `m`, `d`), only for replicated, non-player-scoped instances.
- Check: `tools/swirl-rep-test.mjs <port>` with a test place at `/tmp/swirl/fakeassets/assets/9001` (a Script that creates, renames and destroys parts): the unmodified Chocolate codec and DataModel in node end up with the expected Workspace.

## Swirl server joined from a browser (client 1.12)

`swirl-server --place=ID --port=N` now spawns an R6 character per player (Humanoid, Torso, Head with a Head SpecialMesh, arms, legs, body colours, at the first SpawnLocation), sends `after.own` to the owner, accepts `char` messages and relays them to the other players as `ch` records. The wasm client's `entry.js` joins a server on 127.0.0.1 or localhost when the page URL has `swirlws=ws://127.0.0.1:PORT/ws` (it skips the ticket request); this is how a developer tests a local Swirl server from `/play/{id}?swirl=<version>&swirlws=...`. Checked in headless Chromium (`SWIRLWS=ws://127.0.0.1:4313/ws WALK=1 tools/browser-check.cjs 564 1.12 shot.png`): the character spawns, walks and jumps, screenshot `/tmp/swirl/shot-swirlsrv.png`. `tools/swirl-char-test.mjs` checks two node clients see each other's movement. `crates/swirl-lua/build.rs` fails the build if `data/runtime.lua` differs from Chocolate's; `tools/sync-runtime.sh` copies it.
Not done: real ticket redemption, Touched and physics, Humanoid behaviour (health, death, respawn), appearance (shirts, pants, hats), tools, chat, the remaining Lua APIs.

## Server 3: Touched, health, events, tickets

- Server: Touched and TouchEnded from character parts against watched parts (SAT box overlap, `touch.rs`), Humanoid health (Health set from Lua, `TakeDamage`), death and respawn after 5 s (new character, new `own`), `Player:LoadCharacter()`, `Debris:AddItem`, `Players:GetPlayers`, `GetPlayerFromCharacter`, RunService Stepped and Heartbeat, enum values on the wire (`Enum` items for enum properties), Vector2 and UDim2 values. `tools/swirl-touch-test.mjs` with test place 9002 (a kill brick and a Heartbeat counter) shows Died, a health replication, and a respawn with a new character.
- Tickets: `swirl-server --redeem-url=http://127.0.0.1/api/chocolate/redeem --redeem-key-file=FILE` redeems the `ticket=` query value over loopback HTTP, requires `mode: play` and a matching `placeId`, and uses the returned username; an invalid ticket gets a `kick` message. The key is Solar's `X-Chocolate-Key` (the HMAC of `chocolate-redeem` with Solar's secret): the owner has to provide the file, Swirl never reads Solar's config. Tested against a stand-in endpoint only; without the options the server accepts any ticket (development).
- Still missing: tools and gear, chat, RemoteEvent transport (`re` messages), BindableEvent is handled by the runtime itself and untested, async methods (LoadAsset, GetProductInfo, badges), terrain, Humanoid movement commands (MoveTo, Jump), appearance (shirts, pants, hats), the Solar server registry (presence, shutdown, private servers).

## Tools, RemoteEvent, appearance, chat (1.13)

- Server: StarterPack is cloned into each player's Backpack on every spawn; Backpack, PlayerGui and the other scoped containers now replicate to their owner only (`codec::ops_for` and `subtree` take a viewer, `Game::flush_all` builds ops per player, and a joiner skips the journal entries that precede their snapshot, so simultaneous joins no longer lose each other's changes). Messages `equip`, `unequip`, `drop`, `act` (Tool.Activated/Deactivated and the Mouse's Button1Down/Up) and `mouse` match Chocolate's. Equip welds the Handle to the Right Arm with a `RightGrip` Weld (C0 `RIGHT_GRIP_C0`, C1 the Tool's Grip; Grip changes through GripForward/Right/Up/Pos propagate to the weld) and the handle follows the arm on every `char` message, so Handle.Touched works for swords. Loose Tools in Workspace are picked up on touch (equipped at once). `Humanoid:EquipTool/UnequipTools`, Sound Play/Stop (no-ops), `playerFromCharacter`, BasePart.Position and the Tool Grip virtual properties exist. Scripts start when they are parented under Workspace, ServerScriptService or Players (StarterPack scripts no longer run), so cloned tool scripts run. Check: `tools/swirl-tool-test.mjs <port>` on place 1669 (ClassicSword in StarterPack): equip, weld replicated, damage from touch, kill, respawn.
- 2018 era: `swirl-server --era=2018` loads `runtime2018.lua` through the runtime's `extend` (copy in `crates/swirl-lua/data`, checked by build.rs, `tools/sync-runtime.sh`). RemoteEvent: `re` from clients fires `OnServerEvent(player, ...)`, FireClient/FireAllClients send `re {id, a}` with Chocolate's flat wire args (nil, b, n, s, i, v3, c3, cf, v2, ud2, e, l, d); RemoteFunction: `rf {id, k, a}` runs OnServerInvoke and answers `rfr {k, a}`. InvokeClient is not implemented. Check: `tools/swirl-remote-test.mjs <port>` with test place 9004 in `/tmp/swirl/fakeassets` (`--era=2018 --assets=/tmp/swirl/fakeassets`).
- Chat: `chat {m, k}` (all or team) is cleaned (200 chars, control characters dropped), rate limited to one per 0.6 s, fires Player.Chatted and goes out as `chat {p, n, u, m, k}`; the joiner and leave `sys` lines are shown too.
- Appearance: `swirl-server --site=http://127.0.0.1` (loopback only) fetches `/api/charapp/<id>` and the BodyColors endpoint for each joiner (user id from the redeem answer, or the join counter in dev), reads the assets from `<assets>/assets/<id>` or the site, and builds the R6 body with body colours, a BodyColors instance, hats and accessories (HeadWeld from AttachmentPoint, as Chocolate's `attachAccessory` without rig attachments), face decal, head mesh, Shirt, Pants and ShirtGraphic instances. Hats on the server are positioned at spawn only (they are not tracked while the character moves).
- Client 1.13: a weld pass after posing a character (Weld Part0/Part1/C0/C1 from the replicated instances, so Tool handles and hats follow the swinging arm), a hotbar (keys 1 to 9 or click to equip and unequip, left click on the canvas sends `act`, Backspace or Delete drops), a chat box (`/` focuses, Enter sends), clothing drawn as face decals cut from the 585x559 template (vanilla's region tables; pants under shirt under T-shirt), the classic Head SpecialMesh drawn with `c:fonts/head.mesh` and its face decal on a small front quad, and `window.solarRemote` (`send(id, args)` and `take()`) as the RemoteEvent transport for client scripts once the client has a Lua host. Local test without Solar: `DIST=<dist dir> node tools/swirl-local-test.cjs <swirl-server port> <prefix>` serves the dist dir and two headless pages against a local swirl-server (`SCENARIO=look` takes a character screenshot).
- Not done: mouse Hit and Target (the client sends no `mouse` messages), tool animations (Slash and Lunge toolanim), client-side Lua (LocalScripts, CoreScripts, so no GUI, no OnClientEvent handlers), R15 and 2018 characters on the Swirl server, hat tracking and accessory scaling on the server, T-shirt and shirt UV orientation on top and bottom faces unchecked, packages and CharacterMesh.
