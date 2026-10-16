# lumen plan

**English** | [Español](PLAN.es.md)

A Windows desktop app that monitors, cleans, optimizes and checks the security of your PC.
Open source, honest (no snake oil), portable, fast, and light on resources.

## Principles (non-negotiable)
1. **Light:** < 20 MB of RAM at idle, executable < 15 MB, startup < 300 ms. CI checks this.
2. **Reversible:** every action that changes the system is recorded and can be undone.
3. **Transparent:** every action explains what it does and its risk before running.
4. **No snake oil:** no "registry cleaner" and no "RAM booster".
5. **Private and free:** zero telemetry, zero API keys, zero paid services. Analysis is local; the only network use is the optional download of free public signatures and rules.
6. **No background processes** unless the user turns the monitor on.

## Stack
Rust, Slint (software renderer), `sysinfo`, `winreg`, `windows-sys`. JSON persistence in `%APPDATA%\lumen`.
Built with `opt-level = "z"`, `lto`, `strip` and `panic = "abort"`. CI on GitHub Actions.

## Architecture
```
crates/core   UI-free logic: monitor, processes, startup, disk, privacy, actions (journal)
crates/ui     Slint, presentation only
```
Every system change goes through `actions` (`apply()` / `revert()`). Heavy reads run on background threads.

## Phases

### Phase 0 — Foundation (done)
- [x] Cargo workspace and CI with build, clippy and tests.
- [x] Slint window with side navigation.
- [x] RAM and startup measurement in CI.
- [x] Action journal with apply/revert and tests.

### Phase 1 — MVP (done, v0.1)
- [x] Monitor: CPU, RAM, disk and network.
- [x] Processes: sortable list, path and close.
- [x] Startup: reversible enable/disable.
- [x] Disk: temp files, Chrome/Edge caches, space map.
- [ ] Pending: GPU, configurable refresh, digital signature (comes with Phase 3), recycle bin and Firefox.

### Phase 2 — Optimization (in progress)
- [x] Privacy: reversible Windows settings.
- [ ] Uninstaller with leftover cleanup.
- [ ] Services with a curated "safe to disable" database.
- [ ] Profiles: game mode and battery mode.
- [ ] Automatic restore point before big changes.

### Phase 3 — Local, free security check
Layered detection, all offline: Windows Defender, known hashes (abuse.ch, LOLDrivers), YARA rules (`yara-x`),
optional ClamAV (separate download, GPL) and our own scored heuristics. Reversible quarantine.
Fixed wording: "Security check, not a replacement for antivirus". Never claim "PC is clean".
Check each source's license before integrating it. Never handle real malware outside a VM.

### Phase 4 — Health and polish
SMART, battery, temperatures, drivers (read-only), Spanish/English UI, signed executable, documentation.

## Risks
| Risk | Mitigation |
|---|---|
| Antivirus false positives on the app | Sign the executable, open source, CI builds |
| Breaking the user's system | Undo journal, restore point, confirmations |
| Disabling critical services | Curated database; critical ones cannot be touched |
| Promising security that isn't there | "Warning signs" wording, never "clean" |
| Feature bloat | CI measures RAM and startup and fails when over the limit |

## Development rules
- No heavy dependencies without justification (check the binary size impact).
- Every function that changes the system goes in `core/actions` with its `revert()` and tests.
- No API keys or paid services. Never upload user files anywhere.
- `cargo clippy --all-targets -- -D warnings` and tests before every commit.
- Short commits, one per feature.
