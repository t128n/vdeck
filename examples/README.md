# vDeck Plugin Starters (`vdeck-plugin/1`)

Minimal starter plugins implementing `hello` (with `config` and `plugin_config`), `call` (`ping`, `toggle_state`, `dynamic_deck`), and `goodbye` over newline-delimited JSON on stdin/stdout.

## Files

- [plugin_starter.py](plugin_starter.py) — Python 3 (stdlib only)
- [plugin_starter.ts](plugin_starter.ts) — TypeScript (Bun, Deno, or Node 22+ `--experimental-strip-types`)
- [plugin_starter.ps1](plugin_starter.ps1) — PowerShell
- [plugin_starter.sh](plugin_starter.sh) — Bash (requires `jq`)

## Wiring in `vdeck.toml`

```toml
[[plugins]]
id = "example_py"
command = "uv"
args = ["run", "python", "examples/plugin_starter.py"]
config = { greeting = "Hello from vdeck.toml" }

# TypeScript (Bun):
# [[plugins]]
# id = "example_ts"
# command = "bun"
# args = ["examples/plugin_starter.ts"]
# config = { greeting = "Hello from TS" }

# PowerShell:
# [[plugins]]
# id = "example_ps1"
# command = "pwsh"
# args = ["-NoProfile", "-File", "examples/plugin_starter.ps1"]
# config = { greeting = "Hello from PowerShell" }

# Bash:
# [[plugins]]
# id = "example_sh"
# command = "bash"
# args = ["examples/plugin_starter.sh"]
# config = { greeting = "Hello from Bash" }
```

Use any of their commands on a key:

```toml
[[decks.layers.keys]]
label = "Example"
icon = "folder"
accent = "cyan"
action = { type = "plugin", plugin = "example_py", command = "dynamic_deck" }
```
