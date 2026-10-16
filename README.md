# lumen

**English** | [Español](README.es.md)

A lightweight, honest PC optimizer for Windows. One portable `.exe` (~8 MB), ~6 MB of private RAM, opens in ~35 ms.
No telemetry, no accounts, no paid services.

## What it does (v0.5)
Every section shows each option with a checkbox, a short explanation and a risk tag (green = safe, yellow = review). Yellow options start unchecked and advanced ones hide behind "More options". Before applying, lumen shows exactly what it will do, creates a Windows restore point (once per session) and saves each setting's original value. It asks for administrator rights when it opens.
- **Home:** your PC's state (space to free, startup apps, memory), an "Analyze" button and three presets: Minimal, Recommended and Advanced. Below, your hardware with live CPU and GPU.
- **Cleanup:** temp files, browser caches and Windows Update leftovers go to a stash for 7 days (undoable); a duplicate finder; and a disk analyzer with bars.
- **Performance:** power plan, game mode, startup apps (with their icons), services almost nobody uses, visual effects and background apps.
- **Privacy:** telemetry, ads, Start menu suggestions and removal of preinstalled apps (this one can't be undone).
- **Network:** clear the DNS cache, reset the network and pick a DNS (Cloudflare, Google or automatic).
- **Repair:** SFC, DISM, repair Windows Update and disk health (SMART).
- **Updates:** default, security only or paused.
- **History:** every change lumen made, with an undo button on each.
- **More:** processes (tap one to close it), programs (run the uninstaller and clean leftovers), security (Windows Defender plus a behavior review that spots what it can miss) and settings (theme, refresh rate, Spanish or English).
lumen never installs programs, has no "registry cleaner" and no "RAM booster".

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
