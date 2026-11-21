# Bootstrapper research

Goal: understand how Roblox's launcher ("bootstrapper") looked and behaved in the 2012 and 2018 eras, then build Solar's own two launchers in that spirit. Nothing from Roblox is copied: no logo, art, icons, fonts or exact strings. Solar uses its own logo (`/var/www/html/images/solarized.png`), Liberation Sans (SIL OFL) and its own wording.

## What the sources confirm

Public sources are thin on the window itself (no official documentation exists, and archived screenshots of the launcher are rare), so the behaviour below is split into confirmed and recalled.

Confirmed by public sources:

- The launcher is a small separate executable (`RobloxPlayerLauncher.exe` in the 2012 era, version 1.6.x builds in sandbox reports) that downloads and starts the real client, which is a different exe. Sources: [Roblox fandom, Roblox Player](https://roblox.fandom.com/wiki/Roblox_Player), [ANY.RUN report of a 2012 build](https://any.run/report/be3602699a99d51a1b552c4eccb04cce4be1624c2c4845c55a2122afb61e9fa5/44db5552-58fc-41a2-a9a3-d695d6ad5997).
- Install is per user under `%LOCALAPPDATA%\Roblox`, every build in `Versions\<version id>\`, downloads cached in `Downloads\`. The launcher compares the latest version from a version API with the installed one and updates when they differ. Source: [Bloxstrap wiki, deep dive on how the Roblox bootstrapper works](https://github.com/pizzaboxer/bloxstrap/wiki/A-deep-dive-on-how-the-Roblox-bootstrapper-works).
- The launcher updates itself first when the server's bootstrapper version differs from its own, then installs the client. Same source.
- Web links use a custom protocol registered in `HKCU\SOFTWARE\Classes\<scheme>\shell\open\command` with `%1` replaced by the launch URI; the launcher parses it, starts the player with launch flags, waits for the player's start signal, then closes. Same source.
- Status wording seen by users: "Getting the latest Roblox..." (a forum report of it hanging with an endless marquee bar, [DevForum thread](https://devforum.roblox.com/t/getting-the-latest-roblox-infinite-loop-cant-even-uninstall-it/35536)), "Installing Roblox" ([DevForum](https://devforum.roblox.com/t/roblox-not-installing-completely/3810020)) and a popup "Roblox is starting" when pressing Play ([DevForum](https://devforum.roblox.com/t/roblox-player-will-not-launch/2424667)). The progress bar was an indeterminate marquee while checking and a determinate bar while downloading.

Recalled from memory of the original clients (not verifiable from the sources above, treat as general shape):

- 2012: a small fixed-size standard window with a title bar, the product logo at the top, one line of status text, a glossy green-ish progress bar and a Cancel button. Sequence: connecting, checking for updates, downloading/installing, configuring, starting, then the window closes when the game window appears.
- 2018: flatter and cleaner. A small mostly white window, a large logo in the centre, a single line "... is starting", a thin flat progress bar and little else; no title-bar chrome beyond the OS frame.

## Solar's design (original)

Common behaviour (both eras, one codebase in `crates/swirl-bootstrapper`):

- Per-user install, `%LOCALAPPDATA%\Solar\<era>\Versions\<version>\` on Windows and `~/.local/share/solar/<era>/versions/<version>/` on Linux, downloads in `Downloads\`, state in `current.json`.
- Protocol `solar-2012://` or `solar-2018://` (each launcher registers only its own), registered per user and pointed at the copy of the launcher in the newest version folder; Start Menu shortcut on Windows, `.desktop` file on Linux.
- Status sequence: connecting, looking for updates, downloading (percent), setting up files, preparing, starting. The window closes when the player prints `SOLAR_READY`.
- Errors stay in the window with Retry and Close; Cancel is available while working.

2012 flavour (420x210): light blue-grey vertical gradient body, dark navy header band with the Solar logo and a bold "Solar" wordmark in white, status text left-aligned, a beveled trough with a glossy cyan fill (marquee while indeterminate), a beveled Cancel button bottom right. Wording: "Connecting to Solar...", "Looking for updates...", "Downloading Solar 45%", "Setting up files...", "Preparing Solar...", "Starting Solar...".

2018 flavour (380x250): flat white, a 4 px cyan strip at the top, a 72 px logo centred, centred status text, a 6 px flat cyan bar on a light grey track (sliding block while indeterminate), a plain grey Cancel text link; errors use flat filled and outlined buttons. Wording: "Getting ready", "Checking for the newest Solar", "Downloading 45%", "Installing", "Almost there", "Solar is starting".

Colours: Solar cyan `#1fa8c9` (from the logo), logo ring grey `#444`, navy `#0f2b42`/`#20506f` for the 2012 header, text `#1c2b36` / `#2b3138`.
