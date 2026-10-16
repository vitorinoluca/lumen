# lumen

**English** | [Español](README.es.md)

A lightweight, honest PC optimizer for Windows. One portable `.exe` (~8 MB), ~6 MB of private RAM, opens in ~35 ms.
No telemetry, no accounts, no paid services.

## What it does (v0.2)
- **Panel:** your hardware (Windows, CPU, RAM, GPU, board, disks) with live CPU, memory and network.
- **Processes:** sortable list with path, and a button to close a process (asks first).
- **Startup:** turn startup apps on or off without deleting them (the same mechanism Task Manager uses).
- **Disk:** clean temp files and Chrome/Edge caches, and see what takes the most space.
- **Privacy:** switch off Windows advertising ID, suggestions, Bing in Start search, telemetry and activity history.
- **Profiles:** game mode (high performance plan, closes a list of apps) and battery mode.
- **Programs:** run an app's own uninstaller, then review and clean leftover folders and registry keys.
- **Security:** Windows Defender status and detections, plus a behavior review that spots what Defender can miss: new startup entries, scheduled tasks and services, Winlogon/IFEO/WMI tampering, reverse shells, obfuscated commands, fake system processes and disabled Defender or Windows Update. First scan saves a baseline; later scans report only what is new or modified.
- **Services:** a curated list with plain descriptions. Critical ones are locked. Disabling creates a restore point first.

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

## License
MIT
