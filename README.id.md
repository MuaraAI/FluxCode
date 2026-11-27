<!-- Made from README.flux by FluxCode (fluxcode --flux-to-md README.flux README.id.md). Edit README.flux, not this file. -->

# FluxCode

[English](README.md) · **Bahasa Indonesia** · [Changelog](CHANGELOG.md) · [Roadmap](ROADMAP.md) · [Diskusi](https://github.com/MuaraAI/FluxCode/discussions)

Editor kode native berbasis Rust (GPUI) yang dirender GPU untuk Windows, macOS, dan Linux.
Satu binary 16 MB, terbuka 10 ms, 1,4 MB RAM idle, agent AI bawaan, penganalisis disk WizTree, nol telemetri.

[![Latest build](https://img.shields.io/github/v/release/MuaraAI/FluxCode?display_name=tag&label=latest%20build)](https://github.com/MuaraAI/FluxCode/releases/latest)
[![Last commit](https://img.shields.io/github/last-commit/MuaraAI/FluxCode?label=last%20commit)](https://github.com/MuaraAI/FluxCode/commits/main)
[![Build](https://img.shields.io/github/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/MuaraAI/FluxCode/actions/workflows/build.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

<a href="assets/screenshots/agent-workspace.png"><img src="assets/screenshots/hero.id.jpg" width="100%" alt="Workspace agent FluxCode: daftar chat di kiri, percakapan di tengah, dan diff file di kanan."></a>

## Keunggulan Utama

| Fitur | Ringkasan |
| --- | --- |
| **Cepat & Ringan** | Rust + GPUI, binary 16 MB, buka 10 ms, 1,4 MB RAM idle (1 proses) |
| **Agent AI (ACP)** | OpenCode 2, Claude, Codex, Gemini, Copilot; antrean FIFO, Force Send (`Ctrl+Enter`), diff/undo |
| **Optimasi RAM Live** | Pangkas RAM idle agent hingga ~94% (bangun ~25 ms) lewat Windows EcoQoS dan konteks delta |
| **Lifecycle Agent** | Isolasi Job Object, tutup ke tray saat aktif, nol proses zombie saat keluar |
| **WizTree Disk Analyzer** | Treemap Rust bawaan (`Ctrl+Shift+Y`) + konteks AI `@wiztree` dalam 0-1 tool call |
| **Terminal Multibahasa** | Multi-tab dengan klik error `file:baris:kolom` lintas 15+ bahasa |
| **File Non-Teks & Game** | Edit NBT Bedrock/Java, LevelDB, SQLite, PDF, Office, ZIP, dan Hex |
| **Git, Proyek & WSL2** | Grafik commit, cek incoming, 11 manifest proyek bawaan, helper disk cepat WSL2 |
| **Privasi & Pintasan** | Tata letak dan shortcut VS Code, impor tema Marketplace, nol telemetri |

## Benchmark

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/benchmarks/benchmark-dark.svg">
  <img src="assets/benchmarks/benchmark-light.svg" width="100%" alt="Perbandingan benchmark (makin rendah makin baik): FluxCode 10 ms buka jendela, 1,4 MB RAM idle, 72 MB reservasi, 0,05% CPU idle, 1 proses, 16 MB di disk dibanding Zed dan VS Code.">
</picture>

| Metrik (makin rendah makin baik) | FluxCode | Zed 1.27.0 | VS Code 1.145.2 |
| --- | --- | --- | --- |
| Jendela tampil | 10 ms | 818 ms | ~3.000 ms |
| RAM idle (Task Manager) | 1,4 MB | 66,2 MB | 929,2 MB |
| Memori reservasi | 72 MB | 154 MB | 1.315 MB |
| CPU idle (% dari 1 core) | 0,05% | 1,46% | 5,89% |
| Proses | 1 | 2 | 9 |
| Ukuran di disk | 16 MB | 261 MB | 1.044 MB |

*Median 5 kali uji di Windows 11 (Ryzen 5 7535HS, 15 GB RAM, profil kosong, Nov 2026). Data: [`tools/bench/results.json`](tools/bench/results.json) · Skrip: [`tools/bench/bench.ps1`](tools/bench/bench.ps1).*

## FluxCode vs Zed vs VS Code

| Fitur | FluxCode | Zed | VS Code |
| --- | --- | --- | --- |
| Teknologi | Rust + GPUI | Rust + GPUI | TypeScript + Electron |
| Buka / RAM Idle | 10 ms / 1,4 MB | 818 ms / 66,2 MB | ~3.000 ms / 929,2 MB |
| Disk / Proses | 16 MB / 1 | 261 MB / 2 | 1.044 MB / 9 |
| Telemetri | Tidak ada | Opt-out | Opt-out |
| Agent AI | ACP, antrean, Force Send, EcoQoS, Hemat Token | Bawaan + ACP | Copilot + extension |
| Penganalisis Disk | WizTree bawaan + `@wiztree` | Tidak | Hanya extension |
| File Non-Teks & NBT | Edit NBT, LevelDB, SQLite, PDF, Office, Hex | Gambar | Gambar (+ extension) |
| LSP / Debugger | Terjadwal / Run saja | Ya / Ya | Extension / Ya |
| Extension | Tema & grammar + 11 manifest | Ya | Katalog terbesar |
| Remote | Helper cepat WSL2 | SSH | SSH, Container, WSL |
| Lisensi | GPL-3.0-or-later | GPL-3.0 | MIT / Microsoft |

## Unduh

Build terbaru: [Releases](https://github.com/MuaraAI/FluxCode/releases/latest) (rilis berkelanjutan dari `main`, update 1-klik di aplikasi).

| Platform | Paket | Pemasangan |
| --- | --- | --- |
| Windows (x64) | `...-windows-x64-setup.exe` | Jalankan installer (per-user, tanpa admin) |
| Windows Portable | `...-windows-x64.zip` | Ekstrak lalu jalankan `FluxCode.exe` |
| macOS (preview) | `...-macos-universal.dmg` | Tarik ke Applications (**Open Anyway** sekali) |
| Linux (x64, Vulkan) | `...-linux-x64.tar.gz` | Ekstrak lalu jalankan `./install.sh` |

## Pintasan

| Aksi | Tombol | Aksi | Tombol |
| --- | --- | --- | --- |
| Panel Agent AI | `Ctrl+L` | Command Palette | `Ctrl+Shift+P` / `F1` |
| Force Send (Agent) | `Ctrl+Enter` | Cari File / Baris | `Ctrl+P` / `Ctrl+G` |
| Disk Usage (WizTree) | `Ctrl+Shift+Y` | Cari di Semua File | `Ctrl+Shift+F` |
| Terminal / Tab Baru | ``Ctrl+` `` / ``Ctrl+Shift+` `` | Jalankan File / Stop | `F5` / `Shift+F5` |
| Sidebar / Panel | `Ctrl+B` / `Ctrl+J` | Pengaturan / Tema | `Ctrl+,` / `Ctrl+K Ctrl+T` |

## Build & Dokumen

```sh
cargo build --release
cargo test --bin fluxcode
```

| Sumber | Tautan |
| --- | --- |
| Catatan Rilis | [CHANGELOG.md](CHANGELOG.md) |
| Roadmap 2026-2027 | [ROADMAP.md](ROADMAP.md) |
| Kontribusi & CI | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Format Flux | [docs/flux.id.md](docs/flux.id.md) |
| Lisensi | [GPL-3.0-or-later](LICENSE) · [Pemberitahuan Pihak Ketiga](THIRD_PARTY_NOTICES.md) |
