#!/usr/bin/env python3
"""vDeck Hacker News plugin with live top-of-screen news_strip ticker (vdeck-plugin/1)."""

import json
import sys
import urllib.request
import webbrowser
from concurrent.futures import ThreadPoolExecutor

PROTOCOL = "vdeck-plugin/1"
COMMANDS = ["ticker", "stories", "open_story"]
HN_BASE = "https://hacker-news.firebaseio.com/v0"
STORIES_PER_LAYER = 8
PLUGIN_CONFIG: dict = {}


def send(msg: dict) -> None:
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def short_label(text: str, max_len: int = 12) -> str:
    if len(text) <= max_len:
        return text
    return text[: max_len - 1] + "…"


def fetch_json(url: str, timeout: float = 6.0):
    req = urllib.request.Request(url, headers={"User-Agent": "vDeck/0.1"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def fetch_item(item_id: int) -> dict | None:
    try:
        data = fetch_json(f"{HN_BASE}/item/{item_id}.json")
        if isinstance(data, dict) and data.get("title"):
            return data
    except Exception:
        return None
    return None


def fetch_top_stories(limit: int) -> list[dict]:
    ids = fetch_json(f"{HN_BASE}/topstories.json")
    if not isinstance(ids, list):
        raise RuntimeError("unexpected HN response")
    target_ids = [int(i) for i in ids[:limit]]
    with ThreadPoolExecutor(max_workers=8) as pool:
        items = list(pool.map(fetch_item, target_ids))
    return [item for item in items if item is not None]


def build_stories_deck(stories: list[dict]) -> dict:
    layers: list[dict] = []
    pages = max(1, (len(stories) + STORIES_PER_LAYER - 1) // STORIES_PER_LAYER)
    for page in range(pages):
        chunk = stories[page * STORIES_PER_LAYER : (page + 1) * STORIES_PER_LAYER]
        story_iter = iter(chunk)
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
                continue
            item = next(story_iter, None)
            if item is None:
                keys.append(
                    {
                        "label": "",
                        "icon": "circle",
                        "accent": "orange",
                        "action": {"type": "none"},
                    }
                )
                continue
            title = str(item.get("title", "Story"))
            item_id = item.get("id")
            url = str(
                item.get("url") or f"https://news.ycombinator.com/item?id={item_id}"
            )
            keys.append(
                {
                    "label": short_label(title),
                    "icon": "simple-icons:ycombinator",
                    "accent": "orange",
                    "action": {
                        "type": "plugin",
                        "plugin": "hacker_news",
                        "command": "open_story",
                        "params": {"url": url, "title": title},
                    },
                }
            )
        layer_name = f"Top {page + 1}/{pages}" if pages > 1 else "Top"
        layers.append({"name": layer_name, "keys": keys})
    return {"id": "hacker_news", "name": "Hacker News", "layers": layers}


def handle_call(msg_id: int, command: str, active: bool, params: dict) -> dict:
    try:
        limit = int(params.get("limit", PLUGIN_CONFIG.get("limit", 12)))
        speed = float(params.get("speed", PLUGIN_CONFIG.get("speed", 28.0)))

        if command == "ticker":
            if active:
                send({"type": "news_strip", "show": False})
                return {
                    "type": "result",
                    "id": msg_id,
                    "ok": True,
                    "active": False,
                    "status": "HN Ticker Off",
                }
            stories = fetch_top_stories(limit)
            headlines = [
                f"\x1b[97m{item['title']}\x1b[0m \x1b[93m({item.get('score', 0)}P)\x1b[0m"
                for item in stories
            ]
            send(
                {
                    "type": "news_strip",
                    "show": True,
                    "title": "HN",
                    "accent": "orange",
                    "items": headlines,
                    "speed": speed,
                }
            )
            send(
                {
                    "type": "toast",
                    "title": "Hacker News",
                    "detail": f"{len(headlines)} stories",
                    "icon": "simple-icons:ycombinator",
                    "accent": "orange",
                }
            )
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "active": True,
                "status": "HN Ticker On",
            }

        if command == "stories":
            stories = fetch_top_stories(limit)
            deck = build_stories_deck(stories)
            send(
                {
                    "type": "toast",
                    "title": "Hacker News",
                    "detail": f"{len(stories)} stories",
                    "icon": "simple-icons:ycombinator",
                    "accent": "orange",
                }
            )
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "decks": [deck],
                "open_deck": "hacker_news",
                "status": f"{len(stories)} stories",
            }

        if command == "open_story":
            url = str(params.get("url", ""))
            title = str(params.get("title", "Story"))
            if not url:
                return {
                    "type": "result",
                    "id": msg_id,
                    "ok": False,
                    "error": "missing url",
                }
            webbrowser.open(url)
            return {
                "type": "result",
                "id": msg_id,
                "ok": True,
                "status": short_label(title, 20),
            }

        return {
            "type": "result",
            "id": msg_id,
            "ok": False,
            "error": f"unknown command: {command}",
        }
    except Exception as exc:
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
                    "name": "hacker_news",
                    "commands": COMMANDS,
                }
            )
        elif msg_type == "call":
            send(
                handle_call(
                    msg_id=int(msg.get("id", 0)),
                    command=str(msg.get("command", "")),
                    active=bool(msg.get("active", False)),
                    params=msg.get("params") or {},
                )
            )
        elif msg_type == "goodbye":
            break


if __name__ == "__main__":
    main()
