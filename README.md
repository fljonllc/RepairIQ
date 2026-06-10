# RepairIQ

**Diagnose before you act.** Safely reclaim Mac storage without breaking anything.

## V1 Mission

Help users safely reclaim storage without damaging their computer.

## Features

### 1. Deep Storage Scan
Scans all key macOS locations:
- Applications, Documents, Downloads, Desktop
- Library (Caches, Application Support, Logs, Saved State)
- Developer folders (Xcode DerivedData, Archives, CoreSimulator, Cargo, npm)
- Docker containers
- Trash

Output: Total / Used / Free with recovery potential breakdown.

### 2. Storage Story
Instead of "System Data: 164GB", shows:
```
System Data Breakdown
  Developer Cache: 42GB
  Application Support: 38GB
  Logs: 4GB
  Saved State: 2GB
```
Visual pie chart with drill-down.

### 3. Safety Engine
Every item gets classified:
- **Safe** — Can remove immediately (caches, build artifacts, logs)
- **Review** — Needs user review
- **Archive** — Recommend moving elsewhere (old projects)
- **Protected** — Never touch (system files, SSH keys, keychains)

### 4. Visual Explorer
Two viewing modes:
- **List View** — Sorted by size with safety indicators, descriptions, and drill-down
- **Treemap View** — DaisyDisk-style visual blocks colored by safety level

Click any category to filter. Click any item to drill deeper.

### 5. Recovery Vault
Nothing is permanently deleted. Items are moved to `~/.repairiq/vault/` with:
- 7 / 14 / 30 day retention (configurable)
- One-click restore anytime
- Automatic purge after expiry
- SQLite-backed metadata

### 6. Smart Archive
Detects old projects (90+ days inactive, 50MB+) across Developer, Projects, Desktop, and Documents:
```
Project: Old AI Prototype
  Last Opened: 11 months ago
  Size: 18GB
  Type: Python
  Status: Archive Recommended
  [Move To External Drive]
```
Copies to external drive, **verifies the copy**, then suggests removal. Original stays untouched until you manually delete it.

## What RepairIQ Will NOT Do

- ❌ Auto-delete files
- ❌ Use sudo
- ❌ Modify system files
- ❌ Clean without approval
- ❌ Touch active projects
- ❌ Optimize memory
- ❌ Become a "one-click speed booster"

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop | Tauri 2 |
| Frontend | React 19 + TypeScript |
| Engine | Rust |
| Storage | SQLite (vault database) |
| Charts | Recharts |
| Icons | Lucide React |

Small footprint. Fast scanning. Mac support. Future Windows/Linux support via Tauri.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| ⌘1 | Dashboard |
| ⌘2 | Explorer |
| ⌘3 | Recovery Vault |
| ⌘4 | Smart Archive |
| ⌘, | Settings |
| ⌘R | Rescan |

## Development

```bash
# Install dependencies
npm install

# Run in development mode (hot-reload)
npm run tauri dev

# Build for production
npm run tauri build
```

## Build Output

After `npm run tauri build`:
- `src-tauri/target/release/bundle/macos/RepairIQ.app` — macOS app bundle
- `src-tauri/target/release/bundle/dmg/RepairIQ_0.1.0_x64.dmg` — distributable DMG

## Architecture

```
src/                          # React frontend
  components/
    Dashboard.tsx             # Hero screen: recovery potential + storage story
    Explorer.tsx              # List view with safety classification
    Treemap.tsx               # Visual block explorer (DaisyDisk-style)
    Vault.tsx                 # Recovery vault management
    SmartArchive.tsx          # Old project detection + external drive archival
    Settings.tsx              # Preferences panel
    ConfirmDialog.tsx         # Confirmation before destructive actions
    Toast.tsx                 # Notification system
  hooks/
    useScanner.ts             # Tauri command bindings (scan, vault, archive)
  types/
    index.ts                  # Shared TypeScript types
  utils.ts                    # Formatters (bytes → human-readable)
  App.tsx                     # Main app shell with routing + keyboard shortcuts
  App.css                     # Complete dark UI theme

src-tauri/src/                # Rust backend
  lib.rs                      # Tauri command handlers
  scanner.rs                  # Storage scanning + safety classification engine
  vault.rs                    # Recovery vault (SQLite-backed, move/restore/purge)
  archive.rs                  # Smart archive (project detection, copy+verify)
```

## Settings

Configurable via the Settings panel (⌘,):
- Default vault retention period (7/14/30 days)
- Minimum size threshold for scan results
- Show/hide protected items
- Auto-scan on launch
- Archive copy verification toggle

## First Milestone

Run RepairIQ on your MacBook Air and explain where the 164GB of System Data comes from. If the app can accurately explain that one mystery and safely help you recover space, you've already built something valuable.

## Privacy

Your data never leaves your machine. No telemetry. No network requests. No cloud.
