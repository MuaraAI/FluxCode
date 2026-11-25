<!-- Made from README.flux by FluxCode (fluxcode --flux-to-md README.flux README.id.md). Edit README.flux, not this file. -->

# FluxCode

[English](README.md) · **Bahasa Indonesia** · [Changelog](CHANGELOG.md) · [Roadmap](ROADMAP.md) · [Diskusi](https://github.com/MuaraAI/FluxCode/discussions)

Editor kode native berbasis Rust (GPUI) yang dirender GPU untuk Windows, macOS, dan Linux.
Satu binary 16 MB, terbuka dalam ~50 ms, 1,4 MB RAM idle, workspace agent AI bawaan, penganalisis disk WizTree, dan tanpa telemetri.

[![Latest build](https://img.shields.io/github/v/release/MuaraAI/FluxCode?display_name=tag&label=latest%20build)](https://github.com/MuaraAI/FluxCode/releases/latest)
[![Last commit](https://img.shields.io/github/last-commit/MuaraAI/FluxCode?label=last%20commit)](https://github.com/MuaraAI/FluxCode/commits/main)
[![Build](https://img.shields.io/github/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/MuaraAI/FluxCode/actions/workflows/build.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)

<a href="assets/screenshots/agent-workspace.png"><img src="assets/screenshots/hero.id.jpg" width="100%" alt="Workspace agent FluxCode: daftar chat di kiri, percakapan di tengah, dan diff file di kanan."></a>

## Keunggulan Utama

| Area | Bawaan FluxCode | Manfaat |
| --- | --- | --- |
| **Ringan & Cepat** | Satu binary Rust + GPUI 16 MB, terbuka ~50 ms, 1,4 MB RAM idle dalam 1 proses. | Tanpa beban Electron/Chromium; tetap ringan saat dibiarkan terbuka. |
| **Agent AI Bawaan (ACP)** | Menjalankan OpenCode 2, Claude Agent, Codex, Gemini CLI, Copilot, dan semua agent registri ACP dengan antrean FIFO, Force Send ber-konteks (`Ctrl+Enter`), diff/undo per giliran, Hemat Token, Browser DevTools MCP, dan auto-resume kuota. | Alur kerja agent lengkap di dalam editor dengan pasang 1-klik dan isolasi Job Object (tutup ke tray saat aktif, nol proses zombie saat keluar). |
| **Optimasi Agent Live** | Pendinginan Newton + trim Working Set teredam kritis + Windows EcoQoS, ditambah deduplikasi konteks delta Shannon dan alokasi konteks Gibbs-Boltzmann. | Menjaga agent seperti OpenCode 2 tetap hidup tanpa auto-disable, memangkas RAM idle hingga ~94% (bangun ~25 ms), dan mencegah pengiriman ulang konteks file yang sama. |
| **Penganalisis Disk WizTree** | Indekser disk paralel Rust murni (`Ctrl+Shift+Y`), treemap interaktif, rincian ekstensi, dan konteks AI `@wiztree` satu-pass. | Agent melihat ukuran folder dan titik boros dalam 0-1 tool call tanpa scan shell manual. |
| **Terminal Multibahasa** | Terminal banyak tab dengan cycling `Tab` inline, koreksi typo, dan tautan klik error `file:baris:kolom` lintas 15+ bahasa (Python, JS/TS, Rust, Go, Java, C/C++, C#, Zig, PowerShell, dll.). | Klik error compiler atau runtime mana saja untuk langsung melompat ke baris dan kolom kode. |
| **File Non-Teks & Game** | Editor SNBT untuk NBT Minecraft Bedrock & Java (`level.dat`, `level.dat_old`, `.mcstructure`, `.nbt`), viewer LevelDB, SQLite, PDF, Word/Excel/PowerPoint, ZIP/JAR, dan Hex. | Buka file biner, database, dan dokumen Office di sebelah kode tanpa extension tambahan. |
| **Git, Proyek & WSL2** | Git bawaan (grafik commit, cek incoming otomatis, PR lewat `gh`), 11 manifest proyek bawaan (Paper, add-on Bedrock, Gradle/Maven via FluxCompile, Rust, Node, Python, Go, C/C++, .NET), helper disk WSL2 cepat, dan pratinjau GPU Markdown/`.flux`. | Langsung jalan di Windows, macOS, dan Linux tanpa konfigurasi plugin. |
| **Tombol VS Code & Privasi** | Tata letak dan pintasan VS Code, impor tema VS Code Marketplace native (dengan penyesuaian kontras minimal 4,5:1), sidebar bisa dipindah kiri/kanan, dan nol telemetri. | Langsung terbiasa tanpa akun wajib dan tanpa pelacakan. |

## Benchmark

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/benchmarks/benchmark-dark.svg">
  <img src="assets/benchmarks/benchmark-light.svg" width="100%" alt="Perbandingan benchmark (makin rendah makin baik): FluxCode ~50 ms buka jendela, 1,4 MB RAM idle, 72 MB reservasi, 0,05% CPU idle, 1 proses, 16 MB di disk dibanding Zed dan VS Code.">
</picture>

| Metrik (makin rendah makin baik) | FluxCode | Zed 1.27.0 | VS Code 1.145.2 |
| --- | --- | --- | --- |
| Jendela tampil | ~50 ms | 818 ms | ~3.000 ms |
| RAM idle (Task Manager) | 1,4 MB | 66,2 MB | 929,2 MB |
| Memori reservasi (commit) | 72 MB | 154 MB | 1.315 MB |
| CPU saat idle (% dari 1 core) | 0,05% | 1,46% | 5,89% |
| Jumlah proses | 1 | 2 | 9 |
| Ukuran di disk | 16 MB | 261 MB | 1.044 MB |

*Median 5 kali uji di Windows 11 (Ryzen 5 7535HS, 15 GB RAM, profil kosong, Nov 2026). Data mentah: [`tools/bench/results.json`](tools/bench/results.json) · Skrip: [`tools/bench/bench.ps1`](tools/bench/bench.ps1).*

## FluxCode vs Zed vs VS Code

| Fitur | FluxCode | Zed | VS Code |
| --- | --- | --- | --- |
| Teknologi | Rust + GPUI | Rust + GPUI | TypeScript + Electron |
| RAM Idle / Disk | 1,4 MB / 16 MB (1 proses) | 66,2 MB / 261 MB (2 proses) | 929,2 MB / 1.044 MB (9 proses) |
| Telemetri | Tidak ada | Aktif bawaan (bisa dimatikan) | Aktif bawaan (bisa dimatikan) |
| Agent AI | Semua agent ACP, antrean FIFO + Force Send, optimasi RAM/EcoQoS live, Hemat Token, Browser MCP, inline Copilot | Agent sendiri + ACP | Copilot + extension |
| Penganalisis Disk Bawaan | Ya (WizTree Rust + konteks AI `@wiztree`) | Tidak | Hanya lewat extension |
| File Non-Teks & Game | Edit NBT Bedrock/Java, LevelDB, SQLite, PDF, Office, ZIP, Hex | Gambar | Gambar (+ extension) |
| Language Server (LSP) | Terjadwal ([Roadmap](ROADMAP.md)) | Bawaan | Lewat extension |
| Debugger | Hanya Run (belum ada debugger) | Ya | Ya |
| Extension | Tema/grammar Marketplace + 11 manifest bawaan (tanpa host JS) | Ya | Katalog terbesar |
| Remote | Helper disk cepat WSL2 | SSH | SSH, Container, WSL |
| Lisensi | GPL-3.0-or-later | GPL-3.0 | MIT (OSS) / Microsoft |

## Unduh

Ambil build terbaru di [Releases](https://github.com/MuaraAI/FluxCode/releases/latest) (rilis berkelanjutan dari `main`, pembaruan 1-klik di aplikasi).

| Platform | Paket | Pemasangan |
| --- | --- | --- |
| Windows 10/11 (x64) | `FluxCode-BuildDev-...-windows-x64-setup.exe` | Jalankan installer (per-user, tanpa hak admin). |
| Windows Portable (x64) | `FluxCode-BuildDev-...-windows-x64.zip` | Ekstrak lalu jalankan `FluxCode.exe`. |
| macOS 11+ (Universal, preview) | `FluxCode-BuildDev-...-macos-universal.dmg` | Tarik ke Applications (buka pertama: **Privacy & Security > Open Anyway**). |
| Linux (x64, Vulkan) | `FluxCode-BuildDev-...-linux-x64.tar.gz` | Ekstrak lalu jalankan `./install.sh` (tanpa root). |

## Pintasan

| Aksi | Tombol | Aksi | Tombol |
| --- | --- | --- | --- |
| Panel Agent AI | `Ctrl+L` | Command Palette | `Ctrl+Shift+P` / `F1` |
| Force Send (Agent) | `Ctrl+Enter` | Cari File / Ke Baris | `Ctrl+P` / `Ctrl+G` |
| Disk Usage (WizTree) | `Ctrl+Shift+Y` | Cari di Semua File | `Ctrl+Shift+F` |
| Terminal / Tab Baru | ``Ctrl+` `` / ``Ctrl+Shift+` `` | Jalankan File / Stop | `F5` / `Shift+F5` |
| Sidebar / Panel Bawah | `Ctrl+B` / `Ctrl+J` | Pengaturan / Tema Berikutnya | `Ctrl+,` / `Ctrl+K Ctrl+T` |

## Build & Dokumen

```sh
cargo build --release
cargo test --bin fluxcode
```

| Sumber | Tautan |
| --- | --- |
| Catatan Rilis | [CHANGELOG.md](CHANGELOG.md) |
| Roadmap 2026-2027 | [ROADMAP.md](ROADMAP.md) |
| Panduan Kontribusi & CI | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Format Dokumen Flux | [docs/flux.id.md](docs/flux.id.md) |
| Lisensi | [GPL-3.0-or-later](LICENSE) · [Pemberitahuan Pihak Ketiga](THIRD_PARTY_NOTICES.md) |
