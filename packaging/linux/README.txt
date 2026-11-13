FluxCode Development for Linux (x86_64)

Install
  tar -xzf FluxCode-BuildDev-*-linux-x64.tar.gz
  cd FluxCode-BuildDev-*-linux-x64
  ./install.sh

It installs for your user only: the binary in ~/.local/share/fluxcode (FluxCode
updates itself there), a link in ~/.local/bin, and an app menu entry.
Uninstall: ./install.sh --uninstall

Needs: glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 and later),
a Vulkan driver, and X11 or Wayland with libxkbcommon. Most desktops have them.
git and curl are used when present (Source Control, updates, GitHub sign-in).

Without installing: ./fluxcode also runs straight from this folder.

What changed: https://github.com/MuaraAI/FluxCode/blob/main/CHANGELOG.md
Report a problem: https://github.com/MuaraAI/FluxCode/issues

FluxCode is free software under GPL-3.0-or-later (see LICENSE in the source).
Copyright (C) 2026 MuaraAI and contributors. https://github.com/MuaraAI/FluxCode
