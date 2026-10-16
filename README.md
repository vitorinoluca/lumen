# lumen

**English** | [Español](README.es.md)

A lightweight, honest PC optimizer for Windows. One portable `.exe` (~8 MB), ~6 MB of private RAM, opens in ~35 ms.
No telemetry, no accounts, no paid services.

## What it does (v0.2)
- **Panel:** your hardware (Windows, CPU, RAM, GPU, board, disks) with live CPU, memory and network.
- **Processes:** sortable list with path, and a button to close a process (asks first).
- **Startup:** turn startup apps on or off without deleting them (the same mechanism Task Manager uses).
- **Disk cleanup:** one button frees temp files and Chrome/Edge caches. Files go to a stash for 7 days, so it can be undone.
- **Optimize:** one simple list (game and battery modes, privacy, services almost nobody uses) and an "Aplicar las recomendadas" button. Risky options stay hidden under "advanced", and critical services are never shown.
- **Programs:** run an app's own uninstaller, then review and clean leftover folders and registry keys.
- **Security:** Windows Defender status and detections, plus a behavior review that spots what Defender can miss: new startup entries, scheduled tasks and services, Winlogon/IFEO/WMI tampering, reverse shells, obfuscated commands, fake system processes and disabled Defender or Windows Update. First scan saves a baseline; later scans report only what is new or modified.
- **Settings:** light, dark or Windows theme, dashboard refresh rate, and Spanish or English. Saved in `%APPDATA%lumennfig.json`.

## How everything is undone
Every change is recorded in `%APPDATA%\lumen\journal.json`. See [docs/UNDO.md](docs/UNDO.md).

## What we do NOT do
No "registry cleaner" and no "RAM booster": they don't help and can break things.

## Build
```
cargo run --release                  # run the app
cargo clippy --all-targets -- -D warnings && cargo test
powershell scripts/check-limits.ps1  # size, RAM and startup limits (also run by CI)
```
Requires Rust and the MSVC build tools.

## Roadmap
See [PLAN.md](docs/PLAN.md). Design rules: [docs/DESIGN.md](docs/DESIGN.md).

## Data sources
The driver check uses LOLDrivers (Apache-2.0), downloaded during a review and refreshed at most once a week. See [docs/SOURCES.md](docs/SOURCES.md).

## License
MIT
