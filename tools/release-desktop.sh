#!/bin/sh
set -e
export PATH=$HOME/.cargo/bin:$PATH
cd /var/www/swirl
era=${1:?usage: release-desktop.sh <2012|2018> <linux-x64|windows-x64> <N.M> [notes]}
platform=${2:?platform}
version=${3:?version N.M}
notes=${4:-"Solar desktop $version"}
case "$platform" in
    linux-x64) target=x86_64-unknown-linux-gnu; exe=""; base=target ;;
    windows-x64) target=x86_64-pc-windows-gnu; exe=".exe"; base=target/x86_64-pc-windows-gnu ;;
    *) echo "platform must be linux-x64 or windows-x64"; exit 1 ;;
esac
out=${OUT:-/var/www/swirl/dist/desktop/$era/$platform}
mkdir -p "$out"
tflag=""
if [ "$platform" = windows-x64 ]; then
    [ -f /var/www/swirl/.toolchain/env.sh ] || sh /var/www/swirl/tools/setup-windows-toolchain.sh
    . /var/www/swirl/.toolchain/env.sh
    tflag="--target $target"
fi
SOLAR_BOOTSTRAPPER_VERSION=$version cargo build --profile launcher $tflag -p swirl-bootstrapper --bin SolarPlayer${era}Launcher
cargo build --profile client $tflag -p swirl-desktop
launcher=$base/launcher/SolarPlayer${era}Launcher$exe
player=$base/client/SolarPlayer$exe
zipdir=$(mktemp -d)
cp "$player" "$zipdir/SolarPlayer$exe"
[ "$platform" = windows-x64 ] && cp $(ls -d $HOME/.cargo/registry/src/*/webview2-com-sys-*/x64 | head -1)/WebView2Loader.dll "$zipdir/WebView2Loader.dll"
zipname=SolarPlayer$era-$platform-$version.zip
rm -f "$out/$zipname"
python3 - "$zipdir" "$out/$zipname" <<'PY'
import sys, os, zipfile
src, dst = sys.argv[1], sys.argv[2]
z = zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED)
for n in sorted(os.listdir(src)):
    i = zipfile.ZipInfo(n)
    i.external_attr = 0o755 << 16
    i.compress_type = zipfile.ZIP_DEFLATED
    z.writestr(i, open(os.path.join(src, n), "rb").read())
z.close()
PY
rm -rf "$zipdir"
cp "$launcher" "$out/SolarPlayer${era}Launcher-$platform-$version$exe"
echo "built $out/$zipname and launcher $version"
if [ -n "$PUBLISH" ]; then
    flag=""
    [ -n "$CURRENT" ] && flag="--current"
    php /var/www/html/tools/release.php publish-desktop developer "$era" "$platform" client "$version" "$out/$zipname" --notes="$notes" $flag
    php /var/www/html/tools/release.php publish-desktop developer "$era" "$platform" bootstrapper "$version" "$out/SolarPlayer${era}Launcher-$platform-$version$exe" --notes="$notes" $flag
fi
