#!/bin/sh
# Pack a FluxCode binary as FluxCode.app, then as a disk image and an update archive:
#   packaging/macos/bundle.sh BuildDev-2026-11-10-21-00-00 target/fluxcode dist
# Output, in the given folder:
#   FluxCode-<build>-macos-universal.dmg     drag FluxCode to Applications
#   FluxCode-<build>-macos-universal.tar.gz  FluxCode.app, for the built-in updater
# Needs macOS (sips, iconutil, hdiutil, codesign).
set -eu

build=$1
bin=$2
out=$3
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
name="FluxCode-$build-macos-universal"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

app="$work/stage/FluxCode.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$out"
install -m 755 "$bin" "$app/Contents/MacOS/fluxcode"

# The icon: every size macOS asks for, from the 1024 px PNG.
set_dir="$work/FluxCode.iconset"
mkdir -p "$set_dir"
for s in 16 32 128 256 512; do
  sips -z $s $s "$root/assets/icon.png" --out "$set_dir/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d "$root/assets/icon.png" --out "$set_dir/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$set_dir" -o "$app/Contents/Resources/FluxCode.icns"

# Numeric versions from the build time: 2026.1110.2100 and 20261110210000.
y=$(echo "$build" | cut -d- -f2); mo=$(echo "$build" | cut -d- -f3); d=$(echo "$build" | cut -d- -f4)
h=$(echo "$build" | cut -d- -f5); mi=$(echo "$build" | cut -d- -f6); s=$(echo "$build" | cut -d- -f7)
short="$y.$mo$d.$h$mi"
long="$y$mo$d$h$mi$s"
sed -e "s/@BUILD@/$build/g" -e "s/@SHORT@/$short/g" -e "s/@LONG@/$long/g" "$here/Info.plist" > "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"

# Not notarized (that needs an Apple Developer ID): signed ad hoc so it runs on Apple
# silicon. The first open asks once (System Settings > Privacy & Security > Open Anyway).
codesign --force --deep --sign - "$app"
codesign --verify --verbose=2 "$app"

# The update archive: one folder holding FluxCode.app.
mkdir -p "$work/tar/$name"
cp -R "$app" "$work/tar/$name/"
# gzip at its strongest: the updater runs `tar -xzf`, which reads any level.
tar -cf - -C "$work/tar" "$name" | gzip -9n > "$out/$name.tar.gz"

# The disk image: FluxCode.app next to a link to Applications.
ln -s /Applications "$work/stage/Applications"
cp "$here/README.txt" "$work/stage/Read me.txt"
# LZMA (ULMO, macOS 10.15+; FluxCode needs 11): about a third smaller than zlib (UDZO).
hdiutil create -volname "FluxCode" -srcfolder "$work/stage" -fs HFS+ -format ULMO -ov "$out/$name.dmg" >/dev/null
ls -lh "$out"
echo "Packed $out/$name.dmg and $out/$name.tar.gz"
