#!/usr/bin/env python3
"""vDeck GitHub plugin using the `gh` CLI (vdeck-plugin/1)."""

import json
import os
import subprocess
import sys
import webbrowser

PROTOCOL = "vdeck-plugin/1"
COMMANDS = ["repos", "open_repo", "ci_logs", "triage"]
REPOS_PER_LAYER = 8
APP_CONFIG: dict = {}
PLUGIN_CONFIG: dict = {}


def send(msg: dict) -> None:
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def short_label(name: str, max_len: int = 12) -> str:
    if len(name) <= max_len:
        return name
    return name[: max_len - 1] + "…"


def empty_key() -> dict:
    return {
        "label": "",
        "icon": "circle",
        "accent": "cyan",
        "action": {"type": "none"},
    }


def build_repos_deck(repos: list[dict]) -> dict:
    layers = []
    if not repos:
        keys = [empty_key() for _ in range(9)]
        keys[6] = {
            "label": "Back",
            "icon": "back",
            "accent": "orange",
            "action": {"type": "back"},
        }
        layers.append({"name": "Empty", "keys": keys})
        return {"id": "github_repos", "name": "GitHub", "layers": layers}

    total_pages = (len(repos) + REPOS_PER_LAYER - 1) // REPOS_PER_LAYER
    for page in range(total_pages):
        chunk = repos[page * REPOS_PER_LAYER : (page + 1) * REPOS_PER_LAYER]
        keys = []
        # Reserve slot 6 (Numpad 1) for Back so every layer has an explicit Back button.
        repo_iter = iter(chunk)
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
                continue
            repo = next(repo_iter, None)
            if repo is None:
                keys.append(empty_key())
                continue
            name = repo.get("name") or repo.get("nameWithOwner") or "repo"
            url = repo.get("url") or ""
            is_private = bool(repo.get("isPrivate", False))
            keys.append(
                {
                    "label": short_label(name),
                    "icon": "lock" if is_private else "simple-icons:github",
                    "accent": "purple" if is_private else "cyan",
                    "action": {
                        "type": "plugin",
                        "plugin": "github",
                        "command": "open_repo",
                        "params": {"url": url, "name": name},
                    },
                }
            )
        layer_name = f"Repos {page + 1}/{total_pages}" if total_pages > 1 else "Repos"
        layers.append({"name": layer_name, "keys": keys})

    return {"id": "github_repos", "name": "GitHub", "layers": layers}


def normalize_hostname(raw: str) -> str:
    host = raw.strip()
    for prefix in ("https://", "http://"):
        if host.lower().startswith(prefix):
            host = host[len(prefix) :]
    return host.split("/", 1)[0].strip()


def fetch_repos(limit: int, owner: str = "", hostname: str = "") -> list[dict]:
    cmd = ["gh", "repo", "list"]
    if owner:
        cmd.append(owner)
    cmd.extend(
        [
            "--limit",
            str(limit),
            "--json",
            "name,nameWithOwner,url,isPrivate",
        ]
    )
    env = os.environ.copy()
    host = normalize_hostname(hostname)
    if host:
        env["GH_HOST"] = host
    try:
        proc = subprocess.run(
            cmd,
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=15,
            check=False,
        )
    except FileNotFoundError as exc:
        raise RuntimeError("gh CLI not found in PATH") from exc

    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout or "gh repo list failed").strip()
        if "gh auth login" in err:
            hint = f"Run 'gh auth login --hostname {host}' first" if host else "Run 'gh auth login' first"
            raise RuntimeError(hint)
        raise RuntimeError(err.splitlines()[0] if err else "gh repo list failed")
    data = json.loads(proc.stdout or "[]")
    if not isinstance(data, list):
        raise RuntimeError("unexpected gh output")  # noqa: TRY004
    return data


def handle_call(msg_id: int, command: str, params: dict) -> dict:
    try:
        if command == "repos":
            limit = int(params.get("limit", PLUGIN_CONFIG.get("limit", 32)))
            owner = str(params.get("owner", PLUGIN_CONFIG.get("owner", "")))
            hostname = str(
                params.get(
                    "hostname",
                    params.get(
                        "host",
                        PLUGIN_CONFIG.get("hostname", PLUGIN_CONFIG.get("host", "")),
                    ),
                )
            )
            repos = fetch_repos(limit, owner, hostname)
            deck = build_repos_deck(repos)
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "decks": [deck],
                "open_deck": "github_repos",
                "status": f"{len(repos)} repos",
            }
        if command == "open_repo":
            url = str(params.get("url", ""))
            name = str(params.get("name", "Repo"))
            if not url:
                return {
                    "type": "result",
                    "id": msg_id,
                    "ok": False,
                    "error": "missing repo url",
                }
            webbrowser.open(url)
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "status": f"Opened {name}",
            }
        if command == "ci_logs":
            hostname = str(
                params.get(
                    "hostname",
                    params.get(
                        "host",
                        PLUGIN_CONFIG.get("hostname", PLUGIN_CONFIG.get("host", "")),
                    ),
                )
            )
            env = os.environ.copy()
            host = normalize_hostname(hostname)
            if host:
                env["GH_HOST"] = host
            proc = subprocess.run(
                ["gh", "run", "list", "--status", "failure", "--limit", "1", "--json", "url,workflowName"],
                env=env,
                capture_output=True,
                text=True,
                encoding="utf-8",
                timeout=15,
                check=False,
            )
            if proc.returncode != 0:
                err = (proc.stderr or proc.stdout or "gh run list failed").strip()
                return {"type": "result", "id": msg_id, "ok": False, "error": err.splitlines()[0]}
            runs = json.loads(proc.stdout or "[]")
            if not runs:
                return {"type": "result", "id": msg_id, "ok": True, "status": "No failed CI runs"}
            url = runs[0].get("url", "")
            wf = runs[0].get("workflowName") or "CI"
            if url:
                webbrowser.open(url)
            return {"type": "result", "id": msg_id, "ok": True, "status": f"Opened {wf}"}
        if command == "triage":
            hostname = str(
                params.get(
                    "hostname",
                    params.get(
                        "host",
                        PLUGIN_CONFIG.get("hostname", PLUGIN_CONFIG.get("host", "")),
                    ),
                )
            )
            env = os.environ.copy()
            host = normalize_hostname(hostname) or "github.com"
            env["GH_HOST"] = host
            proc = subprocess.run(
                ["gh", "issue", "list", "--web"],
                env=env,
                capture_output=True,
                text=True,
                encoding="utf-8",
                timeout=15,
                check=False,
            )
            if proc.returncode != 0:
                webbrowser.open(f"https://{host}/issues")
            return {"type": "result", "id": msg_id, "ok": True, "status": "Opened Issues"}
        return {
            "type": "result",
            "id": msg_id,
            "ok": False,
            "error": f"unknown command: {command}",
        }
    except Exception as exc:  # noqa:  BLE001
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
            APP_CONFIG.clear()
            APP_CONFIG.update(msg.get("config") or {})
            PLUGIN_CONFIG.clear()
            PLUGIN_CONFIG.update(msg.get("plugin_config") or {})
            send(
                {
                    "type": "hello",
                    "protocol": PROTOCOL,
                    "name": "github",
                    "commands": COMMANDS,
                }
            )
        elif msg_type == "call":
            reply = handle_call(
                msg_id=int(msg.get("id", 0)),
                command=str(msg.get("command", "")),
                params=msg.get("params") or {},
            )
            send(reply)
        elif msg_type == "goodbye":
            break


if __name__ == "__main__":
    main()
