#!/bin/sh
# Install FluxCode for the current user (no root needed):
#   ./install.sh              install or update
#   ./install.sh --uninstall  remove it (settings in ~/.config stay)
#
# The binary lives in ~/.local/share/fluxcode so FluxCode can update itself there;
# ~/.local/bin/fluxcode links to it.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
data=${XDG_DATA_HOME:-$HOME/.local/share}
app="$data/fluxcode"
bin="$HOME/.local/bin"
desktop="$data/applications/fluxcode.desktop"
icon="$data/icons/hicolor/512x512/apps/fluxcode.png"

if [ "${1:-}" = "--uninstall" ]; then
  rm -rf "$app" "$bin/fluxcode" "$desktop" "$icon"
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$data/applications" || true
  echo "FluxCode removed."
  exit 0
fi

mkdir -p "$app" "$bin" "$(dirname "$desktop")" "$(dirname "$icon")"
install -m 755 "$here/fluxcode" "$app/fluxcode.new"
mv -f "$app/fluxcode.new" "$app/fluxcode"
ln -sf "$app/fluxcode" "$bin/fluxcode"
install -m 644 "$here/fluxcode.png" "$icon"
sed "s|^Exec=.*|Exec=$app/fluxcode %F|" "$here/fluxcode.desktop" > "$desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$data/applications" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$data/icons/hicolor" || true

echo "Installed FluxCode to $app"
case ":$PATH:" in
  *":$bin:"*) echo "Run it from your app menu or with: fluxcode" ;;
  *) echo "Run it from your app menu. To start it from a terminal, add $bin to PATH." ;;
esac
