use crate::config::{
    AppConfig, DeckConfig, IconKind, KeyAction, LayerConfig, default_config, load_config,
    parse_accent,
};
use crate::icons;
use crate::plugin::{CallTarget, PluginManager};
use rune::{AppHandle, Color, oklch};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct DeckKey {
    pub label: String,
    pub kind: IconKind,
    pub accent: Color,
    pub action: KeyAction,
    pub is_toggle: bool,
    pub close: Option<bool>,
    pub active: bool,
    pub press_count: u32,
}

impl DeckKey {
    /// Returns whether activating this key should close the deck (unless overridden or navigating).
    pub fn should_close(&self) -> bool {
        if let Some(explicit) = self.close {
            return explicit;
        }
        if self.is_toggle {
            return false;
        }
        matches!(
            self.action,
            KeyAction::Trigger | KeyAction::Command { .. } | KeyAction::Plugin { .. }
        )
    }
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub keys: Vec<DeckKey>,
}

#[derive(Clone, Debug)]
pub struct Deck {
    pub id: String,
    pub name: String,
    pub layers: Vec<Layer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeckFrame {
    pub deck_idx: usize,
    pub layer_idx: usize,
}

pub type ToastCallback = Arc<dyn Fn(String, Option<String>, IconKind, Color, bool) + Send + Sync>;
pub type NewsStripCallback =
    Arc<dyn Fn(bool, Option<String>, Color, Vec<String>, Option<f32>, Option<u64>) + Send + Sync>;
pub type StatusIndicatorCallback =
    Arc<dyn Fn(String, bool, Option<String>, IconKind, Color) + Send + Sync>;

pub struct DeckState {
    pub decks: Vec<Deck>,
    pub deck_stack: Vec<DeckFrame>,
    pub last_label: Option<String>,
    pub loading_key: Option<usize>,
    loading_seq: u64,
    pub error_message: Option<String>,
    pub visible: bool,
    pub pressed_key: Option<usize>,
    pub plugins: Arc<PluginManager>,
    active_news_strip_key: Option<CallTarget>,
    last_plugin_target: Option<CallTarget>,
    close_handler: Option<Arc<dyn Fn() + Send + Sync>>,
    toast_handler: Option<ToastCallback>,
    news_strip_handler: Option<NewsStripCallback>,
    status_indicator_handler: Option<StatusIndicatorCallback>,
}

impl Layer {
    fn from_config(layer: LayerConfig) -> Self {
        let mut keys: Vec<DeckKey> = layer
            .keys
            .into_iter()
            .take(9)
            .map(|k| {
                let action = k.action.unwrap_or(if k.toggle {
                    KeyAction::Toggle
                } else {
                    KeyAction::Trigger
                });
                let is_toggle = k.toggle || matches!(action, KeyAction::Toggle);
                DeckKey {
                    label: k.label,
                    kind: k.icon,
                    accent: parse_accent(&k.accent),
                    action,
                    is_toggle,
                    close: k.close,
                    active: false,
                    press_count: 0,
                }
            })
            .collect();

        while keys.len() < 9 {
            keys.push(DeckKey {
                label: "—".to_string(),
                kind: IconKind::Circle,
                accent: oklch(0.55, 0.01, 250.0),
                action: KeyAction::None,
                is_toggle: false,
                close: None,
                active: false,
                press_count: 0,
            });
        }

        Self {
            name: layer.name,
            keys,
        }
    }
}

impl Deck {
    pub fn from_config(deck: DeckConfig) -> Self {
        let mut layers: Vec<Layer> = deck.layers.into_iter().map(Layer::from_config).collect();
        if layers.is_empty() {
            layers.push(Layer::from_config(LayerConfig {
                name: "Base".to_string(),
                keys: Vec::new(),
            }));
        }
        Self {
            id: deck.id,
            name: deck.name,
            layers,
        }
    }
}

impl Default for DeckState {
    fn default() -> Self {
        Self::from_config(load_config())
    }
}

impl DeckState {
    pub fn from_config(cfg: AppConfig) -> Self {
        let plugins = Arc::new(PluginManager::new(cfg.plugins, cfg.raw_json));
        let decks: Vec<Deck> = cfg.decks.into_iter().map(Deck::from_config).collect();

        if decks.is_empty() {
            return Self::from_config(default_config());
        }

        Self {
            decks,
            deck_stack: vec![DeckFrame {
                deck_idx: 0,
                layer_idx: 0,
            }],
            last_label: None,
            loading_key: None,
            loading_seq: 0,
            error_message: None,
            visible: true,
            pressed_key: None,
            plugins,
            active_news_strip_key: None,
            last_plugin_target: None,
            close_handler: None,
            toast_handler: None,
            news_strip_handler: None,
            status_indicator_handler: None,
        }
    }

    pub fn attach_plugins(&self, handle: AppHandle<DeckState>) {
        let repaint_handle = handle.clone();
        icons::register_repaint_hook(move || {
            repaint_handle.update(|_, _| {});
        });
        self.plugins.attach(handle);
    }

    pub fn show_loading_if_seq(&mut self, seq: u64, key_idx: usize) {
        if self.loading_seq == seq {
            self.loading_key = Some(key_idx);
        }
    }

    fn clear_loading(&mut self) {
        self.loading_seq = self.loading_seq.wrapping_add(1);
        self.loading_key = None;
    }

    /// Inserts a new dynamic deck or replaces an existing deck with the same `id`.
    pub fn upsert_deck(&mut self, cfg: DeckConfig) {
        for layer in &cfg.layers {
            for key in &layer.keys {
                icons::prefetch(&key.icon);
            }
        }
        let deck = Deck::from_config(cfg);
        if let Some(existing) = self.decks.iter_mut().find(|d| d.id == deck.id) {
            *existing = deck;
        } else {
            self.decks.push(deck);
        }
    }

    pub fn apply_plugin_decks(&mut self, decks: Vec<DeckConfig>, open_deck: Option<String>) {
        for deck_cfg in decks {
            self.upsert_deck(deck_cfg);
        }
        if let Some(deck_id) = open_deck {
            self.open_deck(&deck_id);
        }
    }

    pub fn apply_plugin_result(
        &mut self,
        target: CallTarget,
        ok: bool,
        active: Option<bool>,
        label: Option<String>,
        status: Option<String>,
        error: Option<String>,
        decks: Vec<DeckConfig>,
        open_deck: Option<String>,
    ) {
        let mut target_should_close = true;
        let mut toast_meta = None;
        if let Some(deck) = self.decks.get_mut(target.deck_idx)
            && let Some(layer) = deck.layers.get_mut(target.layer_idx)
            && let Some(key) = layer.keys.get_mut(target.key_idx)
        {
            if let Some(active) = active {
                key.active = active;
            }
            if let Some(label) = label {
                key.label = label;
            }
            target_should_close = key.should_close();
            toast_meta = Some((key.label.clone(), key.kind.clone(), key.accent, key.active));
        }
        self.clear_loading();
        if ok {
            self.error_message = None;
            if let Some(status) = &status {
                self.last_label = Some(status.clone());
            }
            let navigated = open_deck.is_some();
            self.apply_plugin_decks(decks, open_deck);
            if !self.visible
                && let Some((key_label, kind, accent, is_active)) = toast_meta
            {
                let detail = status.or_else(|| {
                    active.map(|_| if is_active { "On" } else { "Off" }.to_string())
                });
                self.notify_toast(key_label, detail, kind, accent, false);
            }
            if !navigated && target_should_close {
                self.request_close();
            }
        } else {
            let msg = error.unwrap_or_else(|| "Plugin action failed".to_string());
            self.last_label = Some(msg.clone());
            self.error_message = Some(msg.clone());
            if !self.visible
                && let Some((key_label, kind, accent, _)) = toast_meta
            {
                self.notify_toast(key_label, Some(msg), kind, accent, true);
            }
        }
    }

    pub fn apply_plugin_event(
        &mut self,
        deck_id: &str,
        layer_idx: usize,
        key_idx: usize,
        active: Option<bool>,
        label: Option<String>,
        icon: Option<IconKind>,
        status: Option<String>,
    ) {
        if let Some(deck) = self.decks.iter_mut().find(|d| d.id == deck_id)
            && let Some(layer) = deck.layers.get_mut(layer_idx)
            && let Some(key) = layer.keys.get_mut(key_idx)
        {
            if let Some(active) = active {
                key.active = active;
            }
            if let Some(label) = label {
                key.label = label;
            }
            if let Some(icon) = icon {
                icons::prefetch(&icon);
                key.kind = icon;
            }
        }
        if let Some(status) = status {
            self.last_label = Some(status);
        }
    }

    pub fn current_frame(&self) -> DeckFrame {
        *self.deck_stack.last().unwrap_or(&DeckFrame {
            deck_idx: 0,
            layer_idx: 0,
        })
    }

    pub fn current_deck(&self) -> &Deck {
        let frame = self.current_frame();
        &self.decks[frame.deck_idx.min(self.decks.len() - 1)]
    }

    pub fn current_layer(&self) -> &Layer {
        let frame = self.current_frame();
        let deck = self.current_deck();
        &deck.layers[frame.layer_idx.min(deck.layers.len() - 1)]
    }

    pub fn in_subdeck(&self) -> bool {
        self.deck_stack.len() > 1
    }

    /// Opens a subdeck by `id` as the new active root starting at L0.
    pub fn open_deck(&mut self, id: &str) {
        if let Some(target) = self.decks.iter().position(|d| d.id == id)
            && target != self.current_frame().deck_idx
        {
            self.deck_stack.push(DeckFrame {
                deck_idx: target,
                layer_idx: 0,
            });
            self.clear_loading();
            self.error_message = None;
        }
    }

    /// Exits the current subdeck back to the parent deck (no-op at root).
    pub fn pop_deck(&mut self) {
        if self.deck_stack.len() > 1 {
            self.deck_stack.pop();
            self.clear_loading();
            self.error_message = None;
        }
    }

    /// Resets navigation back to the root deck (index 0) and layer 0.
    pub fn reset_to_root(&mut self) {
        self.deck_stack = vec![DeckFrame {
            deck_idx: 0,
            layer_idx: 0,
        }];
        self.clear_loading();
        self.error_message = None;
    }

    /// Increments the layer within the current deck (Numpad +).
    pub fn layer_up(&mut self) {
        let max_layer = self.current_deck().layers.len().saturating_sub(1);
        if let Some(frame) = self.deck_stack.last_mut()
            && frame.layer_idx < max_layer
        {
            frame.layer_idx += 1;
            self.clear_loading();
            self.error_message = None;
        }
    }

    /// Decrements the layer within the current deck (Numpad -).
    /// When already at L0 inside a subdeck, exits back to the parent deck.
    pub fn layer_down(&mut self) {
        let at_base = self.current_frame().layer_idx == 0;
        if at_base {
            self.pop_deck();
        } else if let Some(frame) = self.deck_stack.last_mut() {
            frame.layer_idx -= 1;
            self.clear_loading();
            self.error_message = None;
        }
    }

    /// Registers a callback invoked when a leaf action executes so the host can hide the deck.
    pub fn set_close_handler<F>(&mut self, handler: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.close_handler = Some(Arc::new(handler));
    }

    /// Registers a callback invoked to display a top-of-screen toast when triggered while hidden.
    pub fn set_toast_handler<F>(&mut self, handler: F)
    where
        F: Fn(String, Option<String>, IconKind, Color, bool) + Send + Sync + 'static,
    {
        self.toast_handler = Some(Arc::new(handler));
    }

    /// Registers a callback invoked to show or hide the top-of-screen news strip marquee.
    pub fn set_news_strip_handler<F>(&mut self, handler: F)
    where
        F: Fn(bool, Option<String>, Color, Vec<String>, Option<f32>, Option<u64>)
            + Send
            + Sync
            + 'static,
    {
        self.news_strip_handler = Some(Arc::new(handler));
    }

    /// Registers a callback invoked to show or hide persistent upper-right status indicators.
    pub fn set_status_indicator_handler<F>(&mut self, handler: F)
    where
        F: Fn(String, bool, Option<String>, IconKind, Color) + Send + Sync + 'static,
    {
        self.status_indicator_handler = Some(Arc::new(handler));
    }

    pub fn show_status_indicator(
        &self,
        id: String,
        show: bool,
        label: Option<String>,
        icon: IconKind,
        accent: Color,
    ) {
        if show {
            icons::prefetch(&icon);
        }
        if let Some(handler) = &self.status_indicator_handler {
            handler(id, show, label, icon, accent);
        }
    }

    pub fn show_plugin_toast(
        &self,
        title: String,
        detail: Option<String>,
        kind: IconKind,
        accent: Color,
        is_error: bool,
    ) {
        icons::prefetch(&kind);
        self.notify_toast(title, detail, kind, accent, is_error);
    }

    pub fn show_news_strip(
        &mut self,
        show: bool,
        title: Option<String>,
        accent: Color,
        items: Vec<String>,
        speed: Option<f32>,
        duration_ms: Option<u64>,
    ) {
        if show && !items.is_empty() {
            if let Some(prev) = self.active_news_strip_key
                && Some(prev) != self.last_plugin_target
                && let Some(deck) = self.decks.get_mut(prev.deck_idx)
                && let Some(layer) = deck.layers.get_mut(prev.layer_idx)
                && let Some(key) = layer.keys.get_mut(prev.key_idx)
            {
                key.active = false;
            }
            self.active_news_strip_key = self.last_plugin_target;
        } else {
            self.active_news_strip_key = None;
        }

        if let Some(handler) = &self.news_strip_handler {
            handler(show, title, accent, items, speed, duration_ms);
        }
    }

    fn notify_toast(
        &self,
        title: String,
        detail: Option<String>,
        kind: IconKind,
        accent: Color,
        is_error: bool,
    ) {
        if let Some(handler) = &self.toast_handler {
            handler(title, detail, kind, accent, is_error);
        }
    }

    /// Sets the currently pressed key slot (used for numpad press visual feedback).
    pub fn set_pressed_key(&mut self, idx: Option<usize>) {
        self.pressed_key = idx;
    }

    fn request_close(&self) {
        if let Some(handler) = &self.close_handler {
            handler();
        }
    }

    pub fn press_key(&mut self, idx: usize) {
        let frame = self.current_frame();
        let Some(deck) = self.decks.get_mut(frame.deck_idx) else {
            return;
        };
        let Some(layer) = deck.layers.get_mut(frame.layer_idx) else {
            return;
        };
        let Some(key) = layer.keys.get_mut(idx) else {
            return;
        };
        if key.action == KeyAction::None {
            return;
        }

        key.press_count += 1;
        let label = key.label.clone();
        let active = key.active;
        let kind = key.kind.clone();
        let accent = key.accent;
        let should_close = key.should_close();
        let action = key.action.clone();

        match action {
            KeyAction::Toggle => {
                key.active = !key.active;
                let next_active = key.active;
                self.last_label = Some(label.clone());
                if !self.visible {
                    self.notify_toast(
                        label,
                        Some(if next_active { "On" } else { "Off" }.to_string()),
                        kind,
                        accent,
                        false,
                    );
                }
                if should_close {
                    self.request_close();
                }
            }
            KeyAction::Trigger => {
                self.last_label = Some(label.clone());
                if !self.visible {
                    self.notify_toast(label, None, kind, accent, false);
                }
                if should_close {
                    self.request_close();
                }
            }
            KeyAction::OpenDeck { deck: target } => {
                self.last_label = Some(label.clone());
                self.open_deck(&target);
                if !self.visible {
                    self.notify_toast(
                        self.current_deck().name.clone(),
                        Some(self.current_layer().name.clone()),
                        kind,
                        accent,
                        false,
                    );
                }
            }
            KeyAction::Back => {
                self.last_label = Some(label);
                self.pop_deck();
                if !self.visible {
                    self.notify_toast(
                        self.current_deck().name.clone(),
                        Some(self.current_layer().name.clone()),
                        kind,
                        accent,
                        false,
                    );
                }
            }
            KeyAction::LayerUp => {
                self.last_label = Some(label);
                self.layer_up();
                if !self.visible {
                    self.notify_toast(
                        self.current_deck().name.clone(),
                        Some(self.current_layer().name.clone()),
                        kind,
                        accent,
                        false,
                    );
                }
            }
            KeyAction::LayerDown => {
                self.last_label = Some(label);
                self.layer_down();
                if !self.visible {
                    self.notify_toast(
                        self.current_deck().name.clone(),
                        Some(self.current_layer().name.clone()),
                        kind,
                        accent,
                        false,
                    );
                }
            }
            KeyAction::Command { cmd, args } => {
                self.last_label = Some(label.clone());
                let _ = std::process::Command::new(cmd).args(args).spawn();
                if !self.visible {
                    self.notify_toast(label, Some("Launched".to_string()), kind, accent, false);
                }
                if should_close {
                    self.request_close();
                }
            }
            KeyAction::Plugin {
                plugin,
                command,
                params,
            } => {
                self.last_label = Some(label.clone());
                self.clear_loading();
                self.error_message = None;
                let seq = self.loading_seq;
                let target = CallTarget {
                    deck_idx: frame.deck_idx,
                    layer_idx: frame.layer_idx,
                    key_idx: idx,
                };
                self.last_plugin_target = Some(target);
                if let Err(err) = self.plugins.call(&plugin, &command, active, &params, target) {
                    self.clear_loading();
                    self.last_label = Some(err.clone());
                    self.error_message = Some(err.clone());
                    if !self.visible {
                        self.notify_toast(label, Some(err), kind, accent, true);
                    }
                } else {
                    self.plugins.schedule_loading(seq, idx);
                }
            }
            KeyAction::None => {}
        }
    }
}

/// Maps grid index (0..9, top-left to bottom-right) to physical Numpad key (7-9 row 1, 4-6 row 2, 1-3 row 3).
pub fn numpad_key_for_index(idx: usize) -> u8 {
    match idx {
        0 => 7,
        1 => 8,
        2 => 9,
        3 => 4,
        4 => 5,
        5 => 6,
        6 => 1,
        7 => 2,
        8 => 3,
        _ => 0,
    }
}

/// Maps physical Numpad digit (1..=9) to grid index (0..9): 7-9 -> row 1, 4-6 -> row 2, 1-3 -> row 3.
pub fn index_for_numpad_key(num: u8) -> Option<usize> {
    match num {
        7 => Some(0),
        8 => Some(1),
        9 => Some(2),
        4 => Some(3),
        5 => Some(4),
        6 => Some(5),
        1 => Some(6),
        2 => Some(7),
        3 => Some(8),
        _ => None,
    }
}
