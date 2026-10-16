# lumen

**English** | [Español](README.es.md)

A lightweight, honest PC optimizer for Windows. One portable `.exe` (~8 MB), ~6 MB of private RAM, opens in ~35 ms.
No telemetry, no accounts, no paid services.

## What it does (v0.1)
- **Status:** live CPU, memory, disk and network.
- **Processes:** sortable list with path, and a button to close a process (asks first).
- **Startup:** turn startup apps on or off without deleting them (the same mechanism Task Manager uses).
- **Disk:** clean temp files and Chrome/Edge caches, and see what takes the most space.
- **Privacy:** switch off Windows advertising ID, suggestions, Bing in Start search, telemetry and activity history.

On Windows 11 the window is translucent (Acrylic). On other systems it falls back to a solid background.

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
