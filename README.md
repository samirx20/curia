# Curia

A control room for agent CLIs. Claude Code, Codex and Antigravity accounts run as **teams** on a host machine. A main thread hands work between them, and you follow and approve from a desktop app or your phone.

Curia drives the real, unmodified CLIs; it is not a new agent harness.

Early days: it works on the author's machine and has no release yet.

## What's here

| Folder | What it is |
|---|---|
| `host/` | The host (Rust): drives the CLIs, keeps runs, threads and the inbox in SQLite, serves the link, the phone app, terminals and files |
| `core/` | The shared protocol, and the client with the `curia` command |
| `desktop/` | The desktop app (Qt 6 / QML on a Rust bridge) |
| `web/` | The phone web app (Svelte), served by the host |
| `packaging/` | The Windows package and its installer |

## Building

- Host and `curia`: `cargo build` (a current stable Rust).
- Desktop: Qt 6.12 (MSVC) and Visual Studio 2022 Build Tools, then `pwsh -File desktop/build.ps1 build` (set `CURIA_QT` to your Qt folder).
- Phone app: `npm ci` then `npm run build` in `web/`.

## A Windows package

`pwsh -File packaging/package.ps1` builds everything in release mode and writes `target/package/Curia-<version>-windows.zip`. Unzip it and double-click **Install Curia.cmd**: it installs for the current Windows user only (no admin rights) into `%LOCALAPPDATA%\Programs\Curia`, adds Curia to the Start menu and to Settings > Apps. The desktop app starts its host by itself; the host keeps its data in `%LOCALAPPDATA%\Curia\data`. The package is not signed, so Windows SmartScreen warns the first time.

Third-party code and licences: [ATTRIBUTIONS.md](ATTRIBUTIONS.md).
