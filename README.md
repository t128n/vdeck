# vDeck

A fast, keyboard-first virtual Stream Deck overlay for Windows built with Rust and [Rune](https://github.com/anomalyco/rune), extensible with stdio JSON-line plugins.

## Features

- **3×3 Numpad Overlay**: Opens near your cursor (`Numpad 0`) and maps `Numpad 1–9` directly to the 3×3 grid (`7–9` top row, `4–6` middle, `1–3` bottom).
- **Decks & Layers**: Navigate nested subdecks and switch layers with `Numpad +` / `Numpad -`.
- **Persistent Status Indicators**: Upper-right status pills for persistent states such as Windows DND (`ToastEnabled`), system/mic mute, and Microsoft Teams mute / raise hand.
- **Toasts & News Strip**: Floating toast notifications and an optional bottom ticker strip for plugins.
- **Custom Icons**: Built-in Lucide presets, Iconify icons (`lucide:...`, `mdi:...`, etc.), or local SVG paths.
- **RDP-Aware Hotkeys**: Low-level keyboard hook keeps `Numpad 0` and open deck controls working even when focused inside Remote Desktop (`mstsc.exe`).

## Controls

| Key | Action |
| --- | --- |
| `Numpad 0` | Show / hide vDeck |
| `Numpad 1–9` | Trigger key slot |
| `Numpad +` / `Numpad -` | Next / previous layer |

## Installation

### From GitHub Releases (Windows x86_64)

1. Ensure [`uv`](https://docs.astral.sh/uv/) is installed and on `PATH`.
2. Download `vdeck-windows-x86_64.zip` from the [Releases](../../releases) page and extract it.
3. Run `.\vdeck.exe setup` in the extracted folder:
   - Verifies `uv` is installed (exits with an error if missing).
   - Copies `vdeck.exe` to `~/.local/share/bin/` and ensures `~/.local/share/bin` is on your user `PATH`.
   - Copies `vdeck.toml` to `~/.config/vdeck/config.toml` (if not already present; pass `--force` to overwrite) and syncs `plugins/` to `~/.local/share/vdeck/plugins/`.
   - Registers automatic Windows startup (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run\vDeck`; pass `--no-startup` to skip or `--uninstall` to remove).
   - Launches `vdeck.exe` (pass `--no-launch` to skip).

### Updating (`vdeck update`)

Once `~/.local/share/bin` is on your `PATH`, replace the installed binary (and sync plugins) with:

```powershell
# From the latest GitHub Release
vdeck update --repo <owner/repo>

# Or from a downloaded release zip / local build directory / vdeck.exe
vdeck update --from .\vdeck-windows-x86_64.zip
```

### Build from Source

Prerequisites:
- [Rust](https://rustup.rs/) (`stable`, edition 2024)
- [`uv`](https://docs.astral.sh/uv/) (for Python plugins)

```powershell
git clone <your-repo-url> vDeck
cd vDeck
cargo build --release --locked
```

The compiled binary will be at `target/release/vdeck.exe`.

## Local Development Setup

`mise.toml` defines the project toolchain (`rust = "stable"`, `uv = "latest"`).

### Run in Development

```powershell
cargo run
```

Or with Rune hot-reload (requires the local `vendor/` Rune checkout):

```powershell
mise run dev
```

### Run Checks & Tests

```powershell
# Rust check + unit tests
cargo check --all-targets --locked
cargo test --locked

# Syntax-check Python plugins and examples
$files = Get-ChildItem -Path plugins, examples -Filter *.py -Recurse | ForEach-Object { $_.FullName }
uv run python -m py_compile @files
```

## Configuration & Plugins

vDeck reads `~/.config/vdeck/config.toml` if present, otherwise `./vdeck.toml`. Plugin scripts are resolved from `~/.local/share/vdeck/plugins` and `./plugins`.

### Included Plugins (`plugins/`)

| File | Description |
| --- | --- |
| `core_win11.py` | Windows 11 DND/Focus (`ToastEnabled`), audio/mic mute, media controls, lock, clock, Obsidian daily notes, ShareX capture, terminal/browser launchers |
| `teams.py` | Microsoft Teams meeting controls (mute, camera, raise hand, reactions, blur, share, leave) via local Teams API WebSocket or hotkeys |
| `github.py` | GitHub notifications, PRs, CI workflow logs, and issue triage (`gh` CLI) |
| `rdp.py` | Remote Desktop (`mstsc.exe`) grouped quick-connect with `cmdkey` credentials and maximized windowed launch |
| `powertoys.py` | PowerToys utilities launcher |
| `file_explorer.py` | Dynamic folder/file browser subdecks |
| `hacker_news.py` | Hacker News ticker strip |
| `markets.py` | Markets ticker strip |
| `example.py` | Minimal reference plugin |

### Writing Custom Plugins

Plugins communicate over newline-delimited JSON (`vdeck-plugin/1`) on stdin/stdout. See [examples/README.md](examples/README.md) and the starter templates in `examples/` (`plugin_starter.py`, `plugin_starter.ps1`, `plugin_starter.sh`, `plugin_starter.ts`).

## CI/CD

- **CI** (`.github/workflows/ci.yml`): Runs `cargo check --all-targets --locked`, `cargo test --locked`, and `python -m py_compile` on `windows-latest` for pushes and pull requests.
- **Release** (`.github/workflows/release.yml`): Builds `vdeck.exe`, packages `vdeck-windows-x86_64.zip`, and publishes a GitHub Release on `v*` tags.
