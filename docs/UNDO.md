# How to undo changes

lumen records every change in `%APPDATA%\lumen\journal.json`, so it can be undone even after restarting the app.

| Page | How to undo |
|---|---|
| Startup | Move the switch back. Nothing is deleted; lumen only marks the entry as disabled. |
| Privacy | Move the switch back. lumen restores the value Windows had before (or removes it if it did not exist). |
| Services | Move the switch back. lumen restores the service's previous start type. Takes effect after a reboot. |
| Profiles | Turn the profile off: the previous power plan comes back. Apps closed by game mode are not reopened. |
| Programs | Leftover folders go to the stash and registry keys are saved to a `.reg` backup. "Undo leftover cleanup" restores both. The program's own uninstall cannot be undone (use the restore point). |
| Disk | Files are moved to `%APPDATA%\lumen\stash`, not deleted. "Undo last clean" puts them back. |

## What cannot be undone
- **Empty stash** deletes the files for good. This is when disk space is actually freed.
- **Closing a process** loses whatever that program had unsaved.

## Restore points
Before disabling a service, lumen asks Windows for a restore point (needs administrator and System Protection turned on). Windows creates at most one every 24 hours. If it cannot be created, lumen tells you and still applies the change, which stays undoable from lumen.

## Notes
- Items marked "administrator" (machine-wide startup entries, telemetry) need lumen to run as administrator. Without it the change fails with a message and nothing is modified.
