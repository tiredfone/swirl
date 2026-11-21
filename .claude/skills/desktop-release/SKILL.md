---
name: desktop-release
description: Build and publish the Solar desktop launchers and native client to the developer channel. Use when asked to release, publish or rebuild the desktop app, or to check desktop build times.
---

# Desktop release

- `tools/release-desktop.sh <2012|2018> <linux-x64|windows-x64> <N.M> [notes]` builds the launcher (profile `launcher`: client profile with opt-level s and strip) with `SOLAR_BOOTSTRAPPER_VERSION=<N.M>` and the player (profile `client`), zips `SolarPlayer[.exe]` into `dist/desktop/<era>/<platform>/SolarPlayer<era>-<platform>-<N.M>.zip` and copies the launcher beside it. `PUBLISH=1` then runs `php /var/www/html/tools/release.php publish-desktop developer <era> <platform> client|bootstrapper <N.M> <file>` for both (`CURRENT=1` adds `--current`). Without `PUBLISH` nothing is published.
- Versions are plain N.M starting at 1.0 and the client and launcher of a release share the number. A launcher update is detected by version number only, so bump N.M whenever launcher code changes, and rebuild both eras (they differ only by the bin).
- Windows needs mingw-w64 (`x86_64-w64-mingw32-gcc`) and `rustup target add x86_64-pc-windows-gnu`; the script stops with a message when the linker is missing.
- Before publishing: run the fake server scenarios from the `bootstrapper` skill and one real launch (`/api/v1/client/launch` as a developer, then `SolarPlayer --offscreen` under xvfb), then publish to `developer` only.
- Players download the launcher from the site; the site serves `<base>/clients-dist/desktop/<era>/<platform>/<kind>/<version>/<file>`.
