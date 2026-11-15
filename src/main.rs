//! vDeck — a virtual 3×3 Streamdeck built with Rune.
//! `cargo dev` opens the window; `cargo dev -- --snap vdeck.png` renders headlessly.

mod config;
mod deck;
mod hotkey;
mod icons;
mod news_strip;
mod plugin;
mod setup;
mod status_bar;
mod toast;
mod ui;

use deck::DeckState;
use rune::{App, AppSpec, WindowOptions};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(sub) = args.get(1).map(String::as_str) {
        let res = match sub {
            "setup" => Some(setup::run(&args[2..])),
            "update" => Some(setup::run_update(&args[2..])),
            _ => None,
        };
        if let Some(result) = res {
            if let Err(err) = result {
                eprintln!("vdeck {sub} failed: {err}");
                std::process::exit(1);
            }
            return;
        }
    }

    let spec = AppSpec::<DeckState> {
        scenes: &[(
            "active",
            |s| {
                s.decks[0].layers[0].keys[0].active = true;
                s.decks[0].layers[0].keys[2].active = true;
                s.decks[0].layers[0].keys[6].active = true;
                s.last_label = Some("Record".to_string());
            },
        )],
        ..AppSpec::new("vDeck", ui::WINDOW_SIZE, DeckState::default, ui::view)
    };
    let mut app = App::new(spec).keep_running().configured_options(
        WindowOptions {
            borderless: true,
            transparent: true,
            resizable: false,
            always_on_top: true,
            ..Default::default()
        }
        .into(),
    );

    let toast = toast::ToastController::new(app.overlay(toast::spec(), toast::ToastState::default()));
    let news_strip = news_strip::NewsStripController::new(
        app.overlay(news_strip::spec(), news_strip::NewsStripState::default()),
    );
    let status_bar = status_bar::StatusBarController::new(
        app.overlay(status_bar::spec(), status_bar::StatusBarState::default()),
    );
    let handle = app.handle();
    let plugin_handle = handle.clone();
    let reset_grace_ms = config::load_config().reset_grace_ms;
    hotkey::place_bottom_right(&handle, ui::WINDOW_SIZE);
    let hotkey_handle = hotkey::spawn(handle.clone(), reset_grace_ms, ui::WINDOW_SIZE);
    handle.update(move |s, _| {
        s.attach_plugins(plugin_handle);
        s.set_close_handler(move || hotkey_handle.request_hide());
        s.set_toast_handler(move |title, detail, kind, accent, is_error| {
            toast.show(title, detail, kind, accent, is_error);
        });
        s.set_news_strip_handler(move |show, title, accent, items, speed, duration_ms| {
            news_strip.update_strip(show, title, accent, items, speed, duration_ms);
        });
        s.set_status_indicator_handler(move |id, show, label, icon, accent| {
            status_bar.update_indicator(id, show, label, icon, accent);
        });
    });
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::parse_config;
    use deck::{index_for_numpad_key, numpad_key_for_index};
    use rune::{Env, Runtime};

    #[test]
    fn all_nine_keys_render_in_a_3x3_grid() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(DeckState::default(), ui::view, ui::WINDOW_SIZE, env);
        rt.frame(0.0);

        let mut rects = Vec::new();
        for idx in 0..9 {
            let key = format!("deck-key-{idx}");
            let (rect, _) = rt.presented(&key).expect("deck key should be presented");
            assert!((rect[2] - 104.0).abs() < 0.5, "key {idx} width");
            assert!((rect[3] - 104.0).abs() < 0.5, "key {idx} height");
            rects.push(rect);
        }

        for r in 0..3 {
            let y0 = rects[r * 3][1];
            for c in 0..3 {
                let rect = rects[r * 3 + c];
                assert!((rect[1] - y0).abs() < 0.5, "row {r} y alignment");
                if c > 0 {
                    let prev_x = rects[r * 3 + c - 1][0];
                    assert!(rect[0] > prev_x, "col {c} should be to the right");
                }
            }
            if r > 0 {
                assert!(rects[r * 3][1] > rects[(r - 1) * 3][1], "row {r} below prev");
            }
        }
    }

    #[test]
    fn tapping_a_key_toggles_its_state_and_records_last_pressed() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let cfg = parse_config(
            r#"
[[decks]]
id = "root"
name = "Main"

[[decks.layers]]
name = "Work"

[[decks.layers.keys]]
label = "Focus"
icon = "sun"
accent = "amber"
toggle = true
"#,
        )
        .unwrap();
        let mut rt = Runtime::new(DeckState::from_config(cfg), ui::view, ui::WINDOW_SIZE, env);
        rt.frame(0.0);

        assert!(!rt.state.decks[0].layers[0].keys[0].active);
        let (x, y) = rt.find("deck-key-0").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);

        assert!(rt.state.decks[0].layers[0].keys[0].active);
        assert_eq!(rt.state.last_label.as_deref(), Some("Focus"));
        assert_eq!(rt.state.decks[0].layers[0].keys[0].press_count, 1);
    }

    #[test]
    fn numpad_layout_maps_7_to_9_as_row_1() {
        assert_eq!(index_for_numpad_key(7), Some(0));
        assert_eq!(index_for_numpad_key(8), Some(1));
        assert_eq!(index_for_numpad_key(9), Some(2));
        assert_eq!(index_for_numpad_key(4), Some(3));
        assert_eq!(index_for_numpad_key(5), Some(4));
        assert_eq!(index_for_numpad_key(6), Some(5));
        assert_eq!(index_for_numpad_key(1), Some(6));
        assert_eq!(index_for_numpad_key(2), Some(7));
        assert_eq!(index_for_numpad_key(3), Some(8));
        assert_eq!(index_for_numpad_key(0), None);

        for idx in 0..9 {
            let num = numpad_key_for_index(idx);
            assert_eq!(index_for_numpad_key(num), Some(idx));
        }
    }

    #[test]
    fn decks_and_layers_navigate_independently() {
        let toml_cfg = r#"
[[decks]]
id = "root"
name = "Home"
[[decks.layers]]
name = "L0"
[[decks.layers.keys]]
label = "A"
[[decks.layers.keys]]
label = "B"
[[decks.layers.keys]]
label = "C"
[[decks.layers.keys]]
label = "Dev"
action = { type = "open_deck", deck = "dev" }
[[decks.layers]]
name = "L1"
[[decks.layers.keys]]
label = "Q"

[[decks]]
id = "dev"
name = "Dev"
[[decks.layers]]
name = "Code"
[[decks.layers.keys]]
label = "Run"
[[decks.layers]]
name = "Inspect"
[[decks.layers.keys]]
label = "Logs"
"#;
        let cfg = parse_config(toml_cfg).expect("test toml parses");
        let mut state = DeckState::from_config(cfg);

        // Starts at root deck ("Main"), layer 0 ("Work")
        assert_eq!(state.current_deck().id, "root");
        assert_eq!(state.current_frame().layer_idx, 0);
        assert!(!state.in_subdeck());

        // Numpad + increments layer within root deck (L0 -> L1 "Quick")
        state.layer_up();
        assert_eq!(state.current_deck().id, "root");
        assert_eq!(state.current_frame().layer_idx, 1);

        // Numpad - decrements back to root L0; another - at root L0 is a no-op
        state.layer_down();
        assert_eq!(state.current_frame().layer_idx, 0);
        state.layer_down();
        assert_eq!(state.current_deck().id, "root");
        assert_eq!(state.current_frame().layer_idx, 0);

        // Open "dev" subdeck (index 3 on root L0): becomes new root at L0
        state.press_key(3);
        assert_eq!(state.current_deck().id, "dev");
        assert_eq!(state.current_frame().layer_idx, 0);
        assert!(state.in_subdeck());

        // Numpad + inside Dev steps to Dev L1 ("Inspect")
        state.layer_up();
        assert_eq!(state.current_deck().id, "dev");
        assert_eq!(state.current_frame().layer_idx, 1);

        // Numpad - steps back to Dev L0, then another - at L0 exits Dev back to Main!
        state.layer_down();
        assert_eq!(state.current_deck().id, "dev");
        assert_eq!(state.current_frame().layer_idx, 0);
        state.layer_down();
        assert_eq!(state.current_deck().id, "root");
        assert_eq!(state.current_frame().layer_idx, 0);
        assert!(!state.in_subdeck());
    }

    #[test]
    fn plugin_toml_and_protocol_round_trip() {
        use config::KeyAction;
        use plugin::{CallTarget, HostMsg, PROTOCOL_VERSION, PluginMsg};

        let toml = r#"
[[plugins]]
id = "obs"
command = "python"
args = ["plugins/example.py"]
config = { host = "localhost", port = 4455 }

[[decks]]
id = "root"
name = "Main"

[[decks.layers]]
name = "Core"

[[decks.layers.keys]]
label = "Mic"
icon = "mic"
accent = "red"
action = { type = "plugin", plugin = "obs", command = "toggle_mute", params = { source = "Mic" } }
"#;
        let cfg = parse_config(toml).expect("plugin config parses");
        assert_eq!(cfg.plugins.len(), 1);
        assert_eq!(cfg.plugins[0].id, "obs");
        assert_eq!(cfg.plugins[0].command, "python");
        assert_eq!(cfg.plugins[0].args, vec!["plugins/example.py"]);
        assert_eq!(cfg.plugins[0].config["port"], 4455);
        assert_eq!(cfg.raw_json["plugins"][0]["id"], "obs");
        let plugin_cfg = cfg.plugins[0].config.clone();
        let raw_json = cfg.raw_json.clone();

        let mut state = DeckState::from_config(cfg);
        assert!(matches!(
            state.decks[0].layers[0].keys[0].action,
            KeyAction::Plugin { .. }
        ));

        let call_json = serde_json::to_string(&HostMsg::Call {
            id: 7,
            command: "toggle_mute",
            active: false,
            params: &serde_json::json!({ "source": "Mic" }),
        })
        .unwrap();
        assert!(call_json.contains("\"type\":\"call\""));

        let reply: PluginMsg = serde_json::from_str(
            r#"{"type":"result","id":7,"ok":true,"active":true,"status":"Mic Muted"}"#,
        )
        .unwrap();
        if let PluginMsg::Result {
            ok,
            active,
            label,
            status,
            error,
            decks,
            open_deck,
            ..
        } = reply
        {
            state.apply_plugin_result(
                CallTarget {
                    deck_idx: 0,
                    layer_idx: 0,
                    key_idx: 0,
                },
                ok,
                active,
                label,
                status,
                error,
                decks,
                open_deck,
            );
        }
        assert!(state.decks[0].layers[0].keys[0].active);
        assert_eq!(state.last_label.as_deref(), Some("Mic Muted"));

        let hello_json = serde_json::to_string(&HostMsg::Hello {
            protocol: PROTOCOL_VERSION,
            config: &raw_json,
            plugin_config: &plugin_cfg,
        })
        .unwrap();
        assert!(hello_json.contains("vdeck-plugin/1"));
        assert!(hello_json.contains("\"port\":4455"));
        assert!(hello_json.contains("\"config\""));
        assert!(hello_json.contains("\"plugin_config\""));
    }

    #[test]
    fn reset_to_root_returns_to_root_deck_and_layer_zero() {
        let default_cfg = parse_config(config::DEFAULT_TOML).expect("default toml parses");
        assert_eq!(default_cfg.reset_grace_ms, 2000);

        let toml_cfg = r#"
[[decks]]
id = "root"
name = "Home"
[[decks.layers]]
name = "L0"
[[decks.layers.keys]]
label = "A"
[[decks.layers.keys]]
label = "B"
[[decks.layers.keys]]
label = "C"
[[decks.layers.keys]]
label = "Dev"
action = { type = "open_deck", deck = "dev" }

[[decks]]
id = "dev"
name = "Dev"
[[decks.layers]]
name = "Code"
[[decks.layers.keys]]
label = "Run"
[[decks.layers]]
name = "Inspect"
[[decks.layers.keys]]
label = "Logs"
"#;
        let cfg = parse_config(toml_cfg).expect("test toml parses");
        let mut state = DeckState::from_config(cfg);
        state.press_key(3); // open "dev" subdeck
        state.layer_up(); // move to Dev L1
        assert_eq!(state.current_deck().id, "dev");
        assert_eq!(state.current_frame().layer_idx, 1);

        state.reset_to_root();
        assert_eq!(state.current_deck().id, "root");
        assert_eq!(state.current_frame().layer_idx, 0);
        assert!(!state.in_subdeck());
    }

    #[test]
    fn deck_slides_up_on_spring_when_shown() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut initial = DeckState::default();
        initial.visible = false;
        let mut rt = Runtime::new(initial, ui::view, ui::WINDOW_SIZE, env);
        rt.frame(0.0);

        let shell_y = |rt: &Runtime<DeckState>| rt.presented("deck-shell").unwrap().0[1];
        let hidden_y = shell_y(&rt);

        rt.update(|s, _| s.visible = true);
        for _ in 0..8 {
            rt.frame(1.0 / 60.0);
        }
        let mid_y = shell_y(&rt);

        for _ in 0..60 {
            rt.frame(1.0 / 60.0);
        }
        let shown_y = shell_y(&rt);

        assert!(
            hidden_y > mid_y && mid_y > shown_y,
            "deck slides up: {hidden_y} -> {mid_y} -> {shown_y}"
        );
        assert!(
            (hidden_y - shown_y - 440.0).abs() < 1.0,
            "travels 440pt from below"
        );
    }

    #[test]
    fn leaf_actions_close_deck_and_pressed_key_tracks_slot() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let toml = r#"
[[decks]]
id = "root"
name = "Main"

[[decks.layers]]
name = "Base"
keys = [
  { label = "Toggle", kind = "mic", action = { type = "toggle" } },
  { label = "Ping", kind = "flash", action = { type = "trigger" } },
  { label = "Sub", kind = "folder", action = { type = "open_deck", deck = "sub" } },
  { label = "Next", kind = "next", action = { type = "layer_up" } },
  { label = "Prev", kind = "previous", action = { type = "layer_down" } },
  { label = "Stay", kind = "flash", close = false, action = { type = "trigger" } },
  { label = "ToggleClose", kind = "mic", close = true, action = { type = "toggle" } },
  { label = "", kind = "empty", action = { type = "none" } },
  { label = "", kind = "empty", action = { type = "none" } },
]

[[decks]]
id = "sub"
name = "Sub"

[[decks.layers]]
name = "SubBase"
keys = [
  { label = "Back", kind = "back", action = { type = "back" } },
  { label = "", kind = "empty", action = { type = "none" } },
  { label = "", kind = "empty", action = { type = "none" } },
  { label = "", kind = "empty", action = { type = "none" } },
  { label = "", kind = "empty", action = { type = "none" } },
  { label = "", kind = "empty", action = { type = "none" } },
  { label = "", kind = "empty", action = { type = "none" } },
  { label = "", kind = "empty", action = { type = "none" } },
  { label = "", kind = "empty", action = { type = "none" } },
]
"#;
        let cfg = parse_config(toml).expect("test toml parses");
        let mut state = DeckState::from_config(cfg);
        let closes = Arc::new(AtomicUsize::new(0));
        let closes_ref = Arc::clone(&closes);
        state.set_close_handler(move || {
            closes_ref.fetch_add(1, Ordering::Relaxed);
        });

        // Navigation, empty, toggles, and close=false actions keep the deck open
        state.press_key(3); // layer_up
        state.press_key(4); // layer_down
        state.press_key(7); // none
        state.press_key(2); // open_deck("sub")
        assert_eq!(state.current_deck().id, "sub");
        state.press_key(0); // back
        assert_eq!(state.current_deck().id, "root");
        state.press_key(0); // toggle (stays open by default)
        state.press_key(5); // trigger with close = false
        assert_eq!(closes.load(Ordering::Relaxed), 0);

        // Action buttons and close=true overrides request close
        state.press_key(1); // trigger (closes by default)
        assert_eq!(closes.load(Ordering::Relaxed), 1);
        state.press_key(6); // toggle with close = true
        assert_eq!(closes.load(Ordering::Relaxed), 2);

        // Pressed key tracks active numpad press slot
        assert_eq!(state.pressed_key, None);
        state.set_pressed_key(Some(7));
        assert_eq!(state.pressed_key, Some(7));
        state.set_pressed_key(None);
        assert_eq!(state.pressed_key, None);
    }

    #[test]
    fn plugin_dynamic_decks_upsert_and_open_without_closing() {
        use plugin::{CallTarget, PluginMsg};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let toml_cfg = r#"
[[decks]]
id = "root"
name = "Home"
[[decks.layers]]
name = "Base"
[[decks.layers.keys]]
label = "Repos"
action = { type = "plugin", plugin = "github", command = "repos" }
"#;
        let cfg = parse_config(toml_cfg).expect("test toml parses");
        let mut state = DeckState::from_config(cfg);
        let closes = Arc::new(AtomicUsize::new(0));
        let closes_ref = Arc::clone(&closes);
        state.set_close_handler(move || {
            closes_ref.fetch_add(1, Ordering::Relaxed);
        });

        let msg_json = r#"{
            "type": "result",
            "id": 1,
            "ok": true,
            "status": "2 repos",
            "open_deck": "github_repos",
            "decks": [{
                "id": "github_repos",
                "name": "GitHub",
                "layers": [{
                    "name": "Repos",
                    "keys": [
                        { "label": "vDeck", "icon": "folder", "accent": "cyan", "action": { "type": "plugin", "plugin": "github", "command": "open_repo", "params": { "url": "https://github.com/example/vDeck" } } },
                        { "label": "Back", "icon": "back", "accent": "orange", "action": { "type": "back" } }
                    ]
                }]
            }]
        }"#;
        let msg: PluginMsg = serde_json::from_str(msg_json).expect("dynamic deck result parses");
        if let PluginMsg::Result {
            ok,
            active,
            label,
            status,
            error,
            decks,
            open_deck,
            ..
        } = msg
        {
            state.apply_plugin_result(
                CallTarget {
                    deck_idx: 0,
                    layer_idx: 0,
                    key_idx: 0,
                },
                ok,
                active,
                label,
                status,
                error,
                decks,
                open_deck,
            );
        }

        // Opening a dynamic deck keeps the menu open and switches to the new deck.
        assert_eq!(closes.load(Ordering::Relaxed), 0);
        assert_eq!(state.current_deck().id, "github_repos");
        assert_eq!(state.current_deck().layers[0].keys[0].label, "vDeck");
        assert_eq!(state.current_deck().layers[0].keys.len(), 9);

        // Normal plugin result without `open_deck` closes the menu.
        state.apply_plugin_result(
            CallTarget {
                deck_idx: 0,
                layer_idx: 0,
                key_idx: 0,
            },
            true,
            None,
            None,
            Some("Opened vDeck".to_string()),
            None,
            Vec::new(),
            None,
        );
        assert_eq!(closes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn hidden_key_press_triggers_top_toast_while_visible_suppresses_it() {
        use std::sync::{Arc, Mutex};

        let toml = r#"
[[decks]]
id = "root"
name = "Main"

[[decks.layers]]
name = "Base"
keys = [
  { label = "Mic", kind = "mic", action = { type = "toggle" } },
  { label = "Ping", kind = "flash", action = { type = "trigger" } },
]
"#;
        let cfg = parse_config(toml).expect("test toml parses");
        let mut state = DeckState::from_config(cfg);
        let toasts: Arc<Mutex<Vec<(String, Option<String>, bool)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let toasts_ref = Arc::clone(&toasts);
        state.set_toast_handler(move |title, detail, _kind, _accent, is_error| {
            toasts_ref.lock().unwrap().push((title, detail, is_error));
        });

        // Visible deck does not emit top toast
        state.visible = true;
        state.press_key(0);
        assert!(toasts.lock().unwrap().is_empty());

        // Hidden deck emits top toast describing what happened
        state.visible = false;
        state.press_key(0); // Mic -> Off
        state.press_key(1); // Ping
        let recorded = toasts.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], ("Mic".to_string(), Some("Off".to_string()), false));
        assert_eq!(recorded[1], ("Ping".to_string(), None, false));
    }

    #[test]
    fn plugin_toast_and_news_strip_messages_dispatch_to_handlers() {
        use config::parse_accent;
        use plugin::PluginMsg;
        use std::sync::{Arc, Mutex};

        let toml_cfg = r#"
[[decks]]
id = "root"
name = "Home"
[[decks.layers]]
name = "Base"
[[decks.layers.keys]]
label = "Repos"
action = { type = "plugin", plugin = "github", command = "repos" }
"#;
        let cfg = parse_config(toml_cfg).expect("test toml parses");
        let mut state = DeckState::from_config(cfg);
        let toasts: Arc<Mutex<Vec<(String, Option<String>)>>> = Arc::new(Mutex::new(Vec::new()));
        let strips: Arc<Mutex<Vec<(bool, Option<String>, Vec<String>)>>> =
            Arc::new(Mutex::new(Vec::new()));

        let toasts_ref = Arc::clone(&toasts);
        state.set_toast_handler(move |title, detail, _, _, _| {
            toasts_ref.lock().unwrap().push((title, detail));
        });
        let strips_ref = Arc::clone(&strips);
        state.set_news_strip_handler(move |show, title, _, items, _, _| {
            strips_ref.lock().unwrap().push((show, title, items));
        });

        let toast_msg: PluginMsg = serde_json::from_str(
            r#"{"type":"toast","title":"Hacker News","detail":"3 stories","accent":"orange"}"#,
        )
        .unwrap();
        if let PluginMsg::Toast {
            title,
            detail,
            icon,
            accent,
            error,
        } = toast_msg
        {
            state.show_plugin_toast(
                title,
                detail,
                icon,
                parse_accent(accent.as_deref().unwrap_or("cyan")),
                error,
            );
        }

        let strip_msg: PluginMsg = serde_json::from_str(
            r#"{"type":"news_strip","title":"HN","accent":"orange","items":["Rust 2.0","vDeck rocks"],"speed":80}"#,
        )
        .unwrap();
        if let PluginMsg::NewsStrip {
            show,
            title,
            accent,
            items,
            speed,
            duration_ms,
        } = strip_msg
        {
            state.show_news_strip(
                show.unwrap_or(!items.is_empty()),
                title,
                parse_accent(accent.as_deref().unwrap_or("amber")),
                items,
                speed,
                duration_ms,
            );
        }

        assert_eq!(
            toasts.lock().unwrap().as_slice(),
            &[("Hacker News".to_string(), Some("3 stories".to_string()))]
        );
        assert_eq!(
            strips.lock().unwrap().as_slice(),
            &[(
                true,
                Some("HN".to_string()),
                vec!["Rust 2.0".to_string(), "vDeck rocks".to_string()]
            )]
        );
    }

    #[test]
    fn news_strip_parses_ansi_colors_and_uppercases_text() {
        use news_strip::parse_ansi_spans;
        use rune::color::hex;
        use rune::oklcha;

        let default_color = oklcha(1.0, 0.0, 0.0, 0.92);
        let spans = parse_ansi_spans(
            "\x1b[97ms&p 500\x1b[0m 7,561.67 \x1b[92m+0.40%\x1b[0m \x1b[38;2;255;128;0mhot\x1b[0m",
            default_color,
        );
        assert_eq!(spans[0].text, "S&P 500");
        assert_eq!(spans[1].text, " 7,561.67 ");
        assert_eq!(spans[1].color, default_color);
        assert_eq!(spans[2].text, "+0.40%");
        assert_ne!(spans[2].color, default_color);
        assert_eq!(spans[3].text, " ");
        assert_eq!(spans[4].text, "HOT");
        assert_eq!(spans[4].color, hex("#ff8000").unwrap());
    }

    #[test]
    fn news_strip_deactivates_previous_ticker_button() {
        use plugin::CallTarget;
        use rune::oklch;

        let toml_cfg = r#"
[[decks]]
id = "root"
name = "Home"

[[decks.layers]]
name = "Base"

[[decks.layers.keys]]
label = "HN Ticker"
toggle = true
action = { type = "plugin", plugin = "hacker_news", command = "ticker" }

[[decks.layers.keys]]
label = "Markets"
toggle = true
action = { type = "plugin", plugin = "markets", command = "ticker" }
"#;
        let cfg = parse_config(toml_cfg).expect("ticker toml parses");
        let mut state = DeckState::from_config(cfg);

        // Simulate pressing HN Ticker (deck 0, layer 0, key 0)
        state.press_key(0);
        state.show_news_strip(
            true,
            Some("HN".to_string()),
            oklch(0.72, 0.16, 50.0),
            vec!["Story".to_string()],
            None,
            None,
        );
        state.apply_plugin_result(
            CallTarget {
                deck_idx: 0,
                layer_idx: 0,
                key_idx: 0,
            },
            true,
            Some(true),
            None,
            None,
            None,
            Vec::new(),
            None,
        );
        assert!(state.decks[0].layers[0].keys[0].active);

        // Now press Markets (deck 0, layer 0, key 1); HN Ticker must untoggle!
        state.press_key(1);
        state.show_news_strip(
            true,
            Some("MKT".to_string()),
            oklch(0.72, 0.16, 155.0),
            vec!["S&P 500".to_string()],
            None,
            None,
        );
        state.apply_plugin_result(
            CallTarget {
                deck_idx: 0,
                layer_idx: 0,
                key_idx: 1,
            },
            true,
            Some(true),
            None,
            None,
            None,
            Vec::new(),
            None,
        );
        assert!(!state.decks[0].layers[0].keys[0].active);
        assert!(state.decks[0].layers[0].keys[1].active);
    }

    #[test]
    fn custom_icons_deserialize_from_toml_and_plugin_protocol() {
        let toml_cfg = r#"
[[decks]]
id = "root"
name = "Home"

[[decks.layers]]
name = "Main"

[[decks.layers.keys]]
label = "Access"
icon = "solar:accessibility-bold"

[[decks.layers.keys]]
label = "Local"
icon = "./assets/custom.svg"

[[decks.layers.keys]]
label = "Mic"
icon = "mic"
"#;
        let cfg = config::parse_config(toml_cfg).unwrap();
        let keys = &cfg.decks[0].layers[0].keys;
        assert_eq!(
            keys[0].icon,
            config::IconKind::Custom("solar:accessibility-bold".to_string())
        );
        assert_eq!(
            keys[1].icon,
            config::IconKind::Custom("./assets/custom.svg".to_string())
        );
        assert_eq!(keys[2].icon, config::IconKind::Mic);

        let toast_msg: plugin::PluginMsg = serde_json::from_str(
            r#"{"type":"toast","title":"Plugin","icon":"lucide:rocket","accent":"cyan"}"#,
        )
        .unwrap();
        match toast_msg {
            plugin::PluginMsg::Toast { icon, .. } => {
                assert_eq!(
                    icon,
                    config::IconKind::Custom("lucide:rocket".to_string())
                );
            }
            _ => panic!("expected toast message"),
        }

        let event_msg: plugin::PluginMsg = serde_json::from_str(
            r#"{"type":"event","deck":"root","layer":0,"key":0,"icon":"plugins/badge.svg"}"#,
        )
        .unwrap();
        match event_msg {
            plugin::PluginMsg::Event { icon, .. } => {
                assert_eq!(
                    icon,
                    Some(config::IconKind::Custom("plugins/badge.svg".to_string()))
                );
            }
            _ => panic!("expected event message"),
        }
    }

    #[test]
    fn svg_parser_and_local_svg_icon_resolve() {
        let parsed = icons::parse_svg_to_icon(
            "test:basic",
            r#"<?xml version="1.0"?><svg viewBox='0 0 32 32'><path d='M0 0h32v32H0z'/></svg>"#,
        )
        .expect("valid svg should parse");
        assert_eq!(parsed.view, 32.0);
        assert!(parsed.body.contains("viewBox=\"0 0 32 32\""));

        let shifted = icons::parse_svg_to_icon(
            "test:shifted",
            r#"<svg viewBox="-2 -4 24 24"><circle cx="10" cy="8" r="4"/></svg>"#,
        )
        .expect("shifted viewBox svg should parse");
        assert_eq!(shifted.view, 24.0);
        assert!(shifted.body.contains("translate(2 4)"));

        assert!(icons::parse_svg_to_icon("test:404", "Not found").is_none());

        let tmp_path = std::env::temp_dir().join("vdeck_test_custom_icon.svg");
        std::fs::write(
            &tmp_path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 12h16"/></svg>"#,
        )
        .unwrap();
        let kind = config::IconKind::Custom(tmp_path.to_string_lossy().into_owned());
        let _el = icons::icon_el(&kind, 26.0, rune::oklch(0.90, 0.01, 260.0));
        let _ = std::fs::remove_file(&tmp_path);
    }

    #[test]
    fn status_indicator_messages_dispatch_and_toggle() {
        use std::sync::{Arc, Mutex};

        let mut state = DeckState::default();
        let events: Arc<Mutex<Vec<(String, bool, Option<String>)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let events_ref = Arc::clone(&events);
        state.set_status_indicator_handler(move |id, show, label, _icon, _accent| {
            events_ref.lock().unwrap().push((id, show, label));
        });

        let show_msg: plugin::PluginMsg = serde_json::from_str(
            r#"{"type":"status_indicator","id":"dnd","show":true,"label":"DND","icon":"lucide:bell-off","accent":"amber"}"#,
        )
        .unwrap();
        if let plugin::PluginMsg::StatusIndicator {
            id,
            show,
            label,
            icon,
            accent,
        } = show_msg
        {
            state.show_status_indicator(
                id,
                show.unwrap_or(true),
                label,
                icon,
                config::parse_accent(accent.as_deref().unwrap_or("amber")),
            );
        }

        let hide_msg: plugin::PluginMsg =
            serde_json::from_str(r#"{"type":"status_indicator","id":"dnd","show":false}"#)
                .unwrap();
        if let plugin::PluginMsg::StatusIndicator {
            id,
            show,
            label,
            icon,
            accent,
        } = hide_msg
        {
            state.show_status_indicator(
                id,
                show.unwrap_or(true),
                label,
                icon,
                config::parse_accent(accent.as_deref().unwrap_or("amber")),
            );
        }

        let recorded = events.lock().unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], ("dnd".to_string(), true, Some("DND".to_string())));
        assert_eq!(recorded[1], ("dnd".to_string(), false, None));
    }
}
