#!/bin/sh
# GPUI's system libraries, with a retry and a DNS fallback: the Ubuntu mirrors
# sometimes do not answer, and a flaky resolver should not fail the build.
set -eu
packages="pkg-config clang libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev libxcb1-dev libfontconfig1-dev libfreetype6-dev libvulkan-dev libssl-dev libzstd-dev"
apt_install() {
  sudo apt-get update -qq &&
    sudo apt-get install -y --no-install-recommends $packages
}
apt_install ||
  { sleep 10; apt_install; } ||
  {
    echo "apt cannot reach the mirrors; using 1.1.1.1 and 8.8.8.8" >&2
    printf 'nameserver 1.1.1.1\nnameserver 8.8.8.8\n' | sudo tee /etc/resolv.conf >/dev/null
    sleep 5
    apt_install
  }
