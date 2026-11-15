#!/usr/bin/env bash
# Minimal vDeck plugin starter in Bash (vdeck-plugin/1).
# Requires: jq

set -euo pipefail

if [[ " $* " != *" --stdio "* ]]; then
  echo "run with --stdio" >&2
  exit 2
fi

PROTOCOL="vdeck-plugin/1"
APP_CONFIG="{}"
PLUGIN_CONFIG="{}"

send_json() {
  printf '%s\n' "$1"
}

while IFS= read -r raw_line; do
  line="${raw_line#$'\xef\xbb\xbf'}"
  [[ -z "${line//[[:space:]]/}" ]] && continue

  msg_type="$(jq -r '.type // empty' <<<"$line")"

  case "$msg_type" in
    hello)
      APP_CONFIG="$(jq -c '.config // {}' <<<"$line")"
      PLUGIN_CONFIG="$(jq -c '.plugin_config // {}' <<<"$line")"
      send_json "$(jq -nc --arg proto "$PROTOCOL" '{
        type: "hello",
        protocol: $proto,
        name: "example_sh",
        commands: ["ping", "toggle_state", "dynamic_deck"]
      }')"
      ;;
    call)
      id="$(jq -r '.id // 0' <<<"$line")"
      cmd="$(jq -r '.command // ""' <<<"$line")"
      active="$(jq -r '.active // false' <<<"$line")"

      case "$cmd" in
        ping)
          greeting="$(jq -r '.greeting // "Pong from Bash"' <<<"$PLUGIN_CONFIG")"
          send_json "$(jq -nc --argjson id "$id" --arg status "$greeting" '{
            type: "result",
            id: $id,
            ok: true,
            status: $status
          }')"
          ;;
        toggle_state)
          if [[ "$active" == "true" ]]; then
            next_active=false
            status="Inactive"
          else
            next_active=true
            status="Active"
          fi
          send_json "$(jq -nc --argjson id "$id" --argjson active "$next_active" --arg status "$status" '{
            type: "result",
            id: $id,
            ok: true,
            active: $active,
            status: $status
          }')"
          ;;
        dynamic_deck)
          send_json "$(jq -nc --argjson id "$id" '{
            type: "result",
            id: $id,
            ok: true,
            open_deck: "example_sh_deck",
            decks: [{
              id: "example_sh_deck",
              name: "Example",
              layers: [{
                name: "Items",
                keys: [
                  {
                    label: "Ping",
                    icon: "circle",
                    accent: "cyan",
                    action: { type: "plugin", plugin: "example_sh", command: "ping" }
                  },
                  {
                    label: "Back",
                    icon: "back",
                    accent: "orange",
                    action: { type: "back" }
                  }
                ]
              }]
            }]
          }')"
          ;;
        *)
          send_json "$(jq -nc --argjson id "$id" --arg err "unknown command: $cmd" '{
            type: "result",
            id: $id,
            ok: false,
            error: $err
          }')"
          ;;
      esac
      ;;
    goodbye)
      break
      ;;
  esac
done
