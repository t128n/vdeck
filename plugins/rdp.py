"""Remote Desktop (RDP) quick-connect plugin for vDeck (vdeck-plugin/1).

Configure server groups and credentials in vdeck.toml under [plugins.config]:

  [[plugins]]
  id = "rdp"
  command = "uv"
  args = ["run", "python", "plugins/rdp.py"]

  [plugins.config]
  # Optional defaults inherited by all groups/servers:
  # username = "Administrator"
  # password = ""
  # config_file = "~/.config/vdeck/rdp.json"  # optional external JSON file

  [[plugins.config.groups]]
  name = "Prod"
  accent = "red"
  username = "admin"
  password = "..."
  servers = [
    { name = "Web-01", host = "10.0.1.10" },
    { name = "DB-01", host = "10.0.1.20:3389", admin = true },
  ]
"""

from __future__ import annotations

import ctypes
import json
import os
import subprocess
import sys
import threading
import time
from ctypes import wintypes
from pathlib import Path

PROTOCOL = "vdeck-plugin/1"
COMMANDS = ["groups", "open_group", "connect"]
ITEMS_PER_LAYER = 8
PLUGIN_CONFIG: dict = {}


def send(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def short_label(text: str, max_len: int = 12) -> str:
    if len(text) <= max_len:
        return text
    return text[: max_len - 1] + "…"


def empty_key() -> dict:
    return {
        "label": "",
        "icon": "circle",
        "accent": "cyan",
        "action": {"type": "none"},
    }


def back_key() -> dict:
    return {
        "label": "Back",
        "icon": "back",
        "accent": "orange",
        "action": {"type": "back"},
    }


def load_rdp_config() -> dict:
    cfg = dict(PLUGIN_CONFIG)
    ext = str(cfg.get("config_file", "")).strip()
    if ext:
        path = Path(os.path.expanduser(ext))
        if path.is_file():
            try:
                loaded = json.loads(path.read_text(encoding="utf-8"))
                if isinstance(loaded, dict):
                    cfg.update(loaded)
            except (OSError, json.JSONDecodeError):
                pass
    return cfg


def resolve_groups(cfg: dict) -> list[dict]:
    groups = cfg.get("groups")
    if isinstance(groups, list) and groups:
        return [g for g in groups if isinstance(g, dict)]
    servers = cfg.get("servers")
    if isinstance(servers, list) and servers:
        return [
            {
                "name": "Servers",
                "accent": cfg.get("accent", "cyan"),
                "servers": servers,
            }
        ]
    return []


def paginate_keys(entries: list[dict], deck_id: str, deck_name: str, empty_name: str) -> dict:
    layers: list[dict] = []
    if not entries:
        keys = [empty_key() for _ in range(9)]
        keys[6] = back_key()
        layers.append({"name": empty_name, "keys": keys})
        return {"id": deck_id, "name": deck_name, "layers": layers}

    total_pages = (len(entries) + ITEMS_PER_LAYER - 1) // ITEMS_PER_LAYER
    for page in range(total_pages):
        chunk = entries[page * ITEMS_PER_LAYER : (page + 1) * ITEMS_PER_LAYER]
        it = iter(chunk)
        keys: list[dict] = []
        for slot in range(9):
            if slot == 6:
                keys.append(back_key())
                continue
            item = next(it, None)
            keys.append(item if item is not None else empty_key())
        layer_name = f"Page {page + 1}/{total_pages}" if total_pages > 1 else deck_name
        layers.append({"name": layer_name, "keys": keys})

    return {"id": deck_id, "name": deck_name, "layers": layers}


def build_groups_deck(cfg: dict) -> dict:
    groups = resolve_groups(cfg)
    entries: list[dict] = []
    for idx, grp in enumerate(groups):
        name = str(grp.get("name") or f"Group {idx + 1}")
        accent = str(grp.get("accent") or "cyan")
        entries.append(
            {
                "label": short_label(name),
                "icon": str(grp.get("icon") or "lucide:server"),
                "accent": accent,
                "action": {
                    "type": "plugin",
                    "plugin": "rdp",
                    "command": "open_group",
                    "params": {"group_index": idx, "group_name": name},
                },
            }
        )
    return paginate_keys(entries, "rdp_groups", "Remote", "No Groups")


def build_servers_deck(cfg: dict, group: dict) -> dict:
    default_user = str(cfg.get("username", "")).strip()
    default_pass = str(cfg.get("password", ""))
    grp_name = str(group.get("name") or "Servers")
    grp_accent = str(group.get("accent") or "cyan")
    grp_user = str(group.get("username", default_user)).strip()
    grp_pass = str(group.get("password", default_pass))

    entries: list[dict] = []
    for srv in group.get("servers") or []:
        if not isinstance(srv, dict):
            continue
        host = str(srv.get("host") or "").strip()
        rdp_file = str(srv.get("rdp_file") or "").strip()
        if not host and not rdp_file:
            continue
        name = str(srv.get("name") or host or Path(rdp_file).stem)
        accent = str(srv.get("accent") or grp_accent)
        user = str(srv.get("username", grp_user)).strip()
        password = str(srv.get("password", grp_pass))
        entries.append(
            {
                "label": short_label(name),
                "icon": str(srv.get("icon") or "lucide:monitor"),
                "accent": accent,
                "action": {
                    "type": "plugin",
                    "plugin": "rdp",
                    "command": "connect",
                    "params": {
                        "name": name,
                        "host": host,
                        "rdp_file": rdp_file,
                        "username": user,
                        "password": password,
                        "accent": accent,
                        "fullscreen": bool(srv.get("fullscreen", group.get("fullscreen", cfg.get("fullscreen", False)))),
                        "admin": bool(srv.get("admin", group.get("admin", False))),
                    },
                },
            }
        )
    return paginate_keys(entries, "rdp_servers", grp_name, "Empty")


def store_rdp_credential(host: str, username: str, password: str) -> None:
    if not host or not username:
        return
    # cmdkey expects TERMSRV/<hostname> without :port
    hostname = host.split(":", 1)[0].strip()
    if not hostname:
        return
    cmd = ["cmdkey", f"/generic:TERMSRV/{hostname}", f"/user:{username}"]
    if password:
        cmd.append(f"/pass:{password}")
    subprocess.run(
        cmd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        timeout=5,
        check=False,
    )


def maximize_pid_window(pid: int, timeout_s: float = 8.0) -> None:
    user32 = ctypes.windll.user32
    SW_MAXIMIZE = 3
    enum_proc_t = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        found: list[int] = []

        def _cb(hwnd: int, _lp: int) -> bool:
            if not user32.IsWindowVisible(hwnd):
                return True
            proc_id = wintypes.DWORD()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(proc_id))
            if proc_id.value == pid:
                cls = ctypes.create_unicode_buffer(64)
                user32.GetClassNameW(hwnd, cls, 64)
                if cls.value == "TscShellContainerClass":
                    found.append(hwnd)
                    return False
            return True

        user32.EnumWindows(enum_proc_t(_cb), 0)
        if found:
            user32.ShowWindow(found[0], SW_MAXIMIZE)
            return
        time.sleep(0.15)


def launch_mstsc(params: dict) -> tuple[str, str]:
    host = str(params.get("host") or "").strip()
    rdp_file = str(params.get("rdp_file") or "").strip()
    name = str(params.get("name") or host or "Remote Desktop")
    username = str(params.get("username") or "").strip()
    password = str(params.get("password") or "")
    admin = bool(params.get("admin"))

    if not host and not rdp_file:
        raise ValueError("missing host or rdp_file")

    if host and username:
        store_rdp_credential(host, username, password)

    cmd = ["mstsc.exe"]
    if rdp_file:
        cmd.append(os.path.expanduser(rdp_file))
    else:
        cmd.append(f"/v:{host}")
        if not params.get("fullscreen"):
            user32 = ctypes.windll.user32
            width = max(1280, int(user32.GetSystemMetrics(0) or 1920))
            height = max(720, int(user32.GetSystemMetrics(1) or 1080))
            cmd.extend([f"/w:{width}", f"/h:{height}"])

    if params.get("fullscreen"):
        cmd.append("/f")
    if admin:
        cmd.append("/admin")

    proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if not params.get("fullscreen"):
        threading.Thread(target=maximize_pid_window, args=(proc.pid,), daemon=True).start()
    return name, host or rdp_file


def handle_call(msg_id: int, command: str, params: dict) -> dict:
    try:
        cfg = load_rdp_config()
        if command in ("groups", "menu"):
            groups = resolve_groups(cfg)
            if not groups:
                send(
                    {
                        "type": "toast",
                        "title": "Remote Desktop",
                        "detail": "Add groups in vdeck.toml",
                        "icon": "lucide:server",
                        "accent": "cyan",
                    }
                )
                return {
                    "type": "result",
                    "id": msg_id,
                    "ok": True,
                    "decks": [build_groups_deck(cfg)],
                    "open_deck": "rdp_groups",
                    "status": "No groups",
                }
            if len(groups) == 1:
                deck = build_servers_deck(cfg, groups[0])
                return {
                    "type": "result",
                    "id": msg_id,
                    "ok": True,
                    "decks": [deck],
                    "open_deck": "rdp_servers",
                    "status": str(groups[0].get("name") or "Servers"),
                }
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "decks": [build_groups_deck(cfg)],
                "open_deck": "rdp_groups",
                "status": f"{len(groups)} groups",
            }

        if command == "open_group":
            groups = resolve_groups(cfg)
            idx = int(params.get("group_index", 0))
            if idx < 0 or idx >= len(groups):
                return {"type": "result", "id": msg_id, "ok": False, "error": "Group not found"}
            deck = build_servers_deck(cfg, groups[idx])
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "decks": [deck],
                "open_deck": "rdp_servers",
                "status": str(groups[idx].get("name") or "Servers"),
            }

        if command == "connect":
            name, target = launch_mstsc(params)
            send(
                {
                    "type": "toast",
                    "title": f"RDP: {name}",
                    "detail": target,
                    "icon": "lucide:monitor",
                    "accent": str(params.get("accent") or "cyan"),
                }
            )
            return {"type": "result", "id": msg_id, "ok": True, "status": f"RDP {name}"}

        return {"type": "result", "id": msg_id, "ok": False, "error": f"unknown command: {command}"}
    except Exception as exc:  # noqa: BLE001
        return {"type": "result", "id": msg_id, "ok": False, "error": str(exc)}


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
                    "name": "rdp",
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
