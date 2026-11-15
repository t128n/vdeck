#!/usr/bin/env python3
"""vDeck file explorer plugin starting in $HOME and browsing folders (vdeck-plugin/1)."""

import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

PROTOCOL = "vdeck-plugin/1"
COMMANDS = ["browse", "open_folder"]
FOLDERS_PER_LAYER = 7
PLUGIN_CONFIG: dict = {}


def send(msg: dict) -> None:
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def short_label(name: str, max_len: int = 12) -> str:
    if len(name) <= max_len:
        return name
    return name[: max_len - 1] + "…"


def deck_id_for_path(path: Path) -> str:
    digest = hashlib.sha1(str(path).encode("utf-8")).hexdigest()[:10]
    return f"fs_{digest}"


def empty_key() -> dict:
    return {
        "label": "",
        "icon": "circle",
        "accent": "cyan",
        "action": {"type": "none"},
    }


def list_folders(path: Path, show_hidden: bool) -> list[Path]:
    folders: list[Path] = []
    with os.scandir(path) as entries:
        for entry in entries:
            try:
                if not entry.is_dir(follow_symlinks=False):
                    continue
                if not show_hidden and entry.name.startswith("."):
                    continue
                folders.append(Path(entry.path))
            except OSError:
                continue
    folders.sort(key=lambda p: p.name.casefold())
    return folders


def build_folder_deck(path: Path, folders: list[Path]) -> tuple[str, dict]:
    deck_id = deck_id_for_path(path)
    title = path.name or str(path)
    layers: list[dict] = []

    pages = max(1, (len(folders) + FOLDERS_PER_LAYER - 1) // FOLDERS_PER_LAYER)
    for page in range(pages):
        chunk = folders[page * FOLDERS_PER_LAYER : (page + 1) * FOLDERS_PER_LAYER]
        folder_iter = iter(chunk)
        keys: list[dict] = []
        for slot in range(9):
            if slot == 6:
                keys.append(
                    {
                        "label": "Back",
                        "icon": "back",
                        "accent": "orange",
                        "action": {"type": "back"},
                    }
                )
            elif slot == 8:
                keys.append(
                    {
                        "label": "Open",
                        "icon": "play",
                        "accent": "green",
                        "action": {
                            "type": "plugin",
                            "plugin": "file_explorer",
                            "command": "open_folder",
                            "params": {"path": str(path)},
                        },
                    }
                )
            else:
                sub = next(folder_iter, None)
                if sub is None:
                    keys.append(empty_key())
                else:
                    keys.append(
                        {
                            "label": short_label(sub.name),
                            "icon": "folder",
                            "accent": "cyan",
                            "action": {
                                "type": "plugin",
                                "plugin": "file_explorer",
                                "command": "browse",
                                "params": {"path": str(sub)},
                            },
                        }
                    )
        layer_name = f"{page + 1}/{pages}" if pages > 1 else "Folders"
        layers.append({"name": layer_name, "keys": keys})

    return deck_id, {"id": deck_id, "name": short_label(title, 18), "layers": layers}


def open_in_os(path: Path) -> None:
    if sys.platform == "win32":
        os.startfile(str(path))  # type: ignore[attr-defined]
    elif sys.platform == "darwin":
        subprocess.Popen(["open", str(path)])
    else:
        subprocess.Popen(["xdg-open", str(path)])


def handle_call(msg_id: int, command: str, params: dict) -> dict:
    try:
        default_root = str(PLUGIN_CONFIG.get("root") or Path.home())
        show_hidden = bool(PLUGIN_CONFIG.get("show_hidden", False))

        if command == "browse":
            raw_path = str(params.get("path") or default_root)
            target = Path(os.path.expandvars(os.path.expanduser(raw_path))).resolve()
            if not target.is_dir():
                return {
                    "type": "result",
                    "id": msg_id,
                    "ok": False,
                    "error": f"Not a folder: {target.name}",
                }
            folders = list_folders(target, show_hidden)
            deck_id, deck = build_folder_deck(target, folders)
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "decks": [deck],
                "open_deck": deck_id,
                "status": f"{len(folders)} folders",
            }

        if command == "open_folder":
            raw_path = str(params.get("path") or default_root)
            target = Path(os.path.expandvars(os.path.expanduser(raw_path))).resolve()
            if not target.is_dir():
                return {
                    "type": "result",
                    "id": msg_id,
                    "ok": False,
                    "error": f"Not a folder: {target.name}",
                }
            open_in_os(target)
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "status": f"Opened {target.name or target}",
            }

        return {
            "type": "result",
            "id": msg_id,
            "ok": False,
            "error": f"unknown command: {command}",
        }
    except Exception as exc:
        return {
            "type": "result",
            "id": msg_id,
            "ok": False,
            "error": str(exc),
        }


def main() -> None:
    if "--stdio" not in sys.argv:
        sys.stderr.write("run with --stdio\n")
        sys.exit(2)

    for raw in sys.stdin:
        line = raw.lstrip("\ufeff").strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue

        msg_type = msg.get("type")
        if msg_type == "hello":
            PLUGIN_CONFIG.clear()
            PLUGIN_CONFIG.update(msg.get("plugin_config") or {})
            send(
                {
                    "type": "hello",
                    "protocol": PROTOCOL,
                    "name": "file_explorer",
                    "commands": COMMANDS,
                }
            )
        elif msg_type == "call":
            send(
                handle_call(
                    msg_id=int(msg.get("id", 0)),
                    command=str(msg.get("command", "")),
                    params=msg.get("params") or {},
                )
            )
        elif msg_type == "goodbye":
            break


if __name__ == "__main__":
    main()
