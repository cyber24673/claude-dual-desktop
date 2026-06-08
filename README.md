# Claude Dual

A multi-profile launcher for **Claude Desktop** on Windows. Run multiple Claude
accounts (work, personal, etc.) on the same machine, each with its own isolated
session and login — switch between them from a simple desktop app.

> Built with [Tauri 2](https://tauri.app/) (Rust backend + HTML/JS frontend).

[![Download for Windows](https://img.shields.io/github/v/release/cyber24673/claude-dual-desktop?label=Download%20for%20Windows&style=flat-square)](https://github.com/cyber24673/claude-dual-desktop/releases/latest)

## Why

Claude Desktop on Windows ships as an **MSIX (Microsoft Store) package**, which
virtualizes its `AppData` and is launched by AUMID — so the usual Electron
`--user-data-dir` trick doesn't cleanly separate accounts on its own. Claude Dual
works around this to give each profile its own persistent state.

## Features

- Create named, color-coded profiles
- Launch each profile with its own isolated Claude Desktop session
- See which profiles are currently running (live status, refreshes every 3s)
- Delete profiles you no longer need

## How it works

Each profile stores its identity (auth tokens, local/session storage, IndexedDB,
preferences, etc.) under `~/.claude-desktop-profiles/<id>/`.

- The **primary** instance runs the real MSIX app, activated via the Windows
  `IApplicationActivationManager` COM interface. Profile data is swapped in/out
  of Claude's real userData dir before launch.
- **Secondary** instances run from a local copy of the Claude Desktop app
  (created on first use) launched with `--user-data-dir` so several profiles can
  run at once.
- Running instances are detected by scanning processes and checking lockfiles.

## Installation

> **Requires [Claude Desktop](https://claude.ai/download) to already be installed.**

1. Go to the [**Releases**](../../releases) page.
2. Download the latest installer (`.msi` or `claude-dual-desktop_x.x.x_x64-setup.exe`).
3. Run it and follow the installer.

### "Windows protected your PC" warning

This app is **not code-signed** (signing certificates are costly), so Windows
SmartScreen may warn you on first run. This is expected for small open-source
apps. To continue:

1. Click **More info**
2. Click **Run anyway**

You can review the full source code in this repository to verify what it does.

## Usage

1. Open **Claude Dual**.
2. Click the **+** button to create a profile — give it a name and a color.
3. Click **Open** (Abrir) on a profile to launch Claude Desktop with that
   profile's isolated session.
4. The first time you open a profile, Claude Desktop starts fresh and asks you to
   log in. Each profile keeps its own login.
5. Profile status (running / stopped) updates automatically.
6. To remove a profile, close it first, then click **Delete** (Eliminar).

> The first time you run a *second* profile at the same time, the app makes a
> local copy of Claude Desktop (a few hundred MB), which can take a moment.

## Build requirements

Only needed if you want to compile it yourself (end users can just use the
installer from Releases):

- Windows 10/11
- [Claude Desktop](https://claude.ai/download) installed (MSIX / Store version)
- [Rust](https://www.rust-lang.org/tools/install) (stable) + Cargo
- [Tauri prerequisites](https://tauri.app/start/prerequisites/) (WebView2 — already
  present on most up-to-date Windows installs)

## Build from source

```bash
# Install the Tauri CLI (once)
cargo install tauri-cli --version "^2"

# Run in dev mode
cargo tauri dev

# Build a release installer
cargo tauri build
```

The built app and installer will be in `src-tauri/target/release/`.

> Note: the first launch of a *secondary* profile copies the Claude Desktop app
> locally (~hundreds of MB) and may take a moment.

## Project structure

```
src/                 Frontend (HTML/CSS/JS)
src-tauri/src/
  lib.rs             Tauri commands (list/create/delete/launch/get_running)
  profiles.rs        Profile registry (profiles.json) + data dirs
  launcher.rs        MSIX/COM launch, data swap, process detection
```

## Disclaimer

This is an unofficial, community project and is **not affiliated with or endorsed
by Anthropic**. It manipulates local Claude Desktop data directories on your own
machine; use at your own risk. "Claude" is a trademark of Anthropic.

## License

[MIT](LICENSE)
