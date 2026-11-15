use crate::config::{DeckConfig, IconKind, PluginConfig, parse_accent};
use crate::deck::DeckState;
use rune::AppHandle;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub const PROTOCOL_VERSION: &str = "vdeck-plugin/1";

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMsg<'a> {
    Hello {
        protocol: &'a str,
        config: &'a serde_json::Value,
        plugin_config: &'a serde_json::Value,
    },
    Call {
        id: u64,
        command: &'a str,
        active: bool,
        params: &'a serde_json::Value,
    },
    Goodbye,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginMsg {
    Hello {
        protocol: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        commands: Vec<String>,
    },
    Result {
        id: u64,
        ok: bool,
        #[serde(default)]
        active: Option<bool>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        decks: Vec<DeckConfig>,
        #[serde(default)]
        open_deck: Option<String>,
    },
    Event {
        deck: String,
        layer: usize,
        key: usize,
        #[serde(default)]
        active: Option<bool>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        icon: Option<IconKind>,
        #[serde(default)]
        status: Option<String>,
    },
    UpsertDecks {
        decks: Vec<DeckConfig>,
        #[serde(default)]
        open_deck: Option<String>,
    },
    Toast {
        title: String,
        #[serde(default)]
        detail: Option<String>,
        #[serde(default)]
        icon: IconKind,
        #[serde(default)]
        accent: Option<String>,
        #[serde(default)]
        error: bool,
    },
    NewsStrip {
        #[serde(default)]
        show: Option<bool>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        accent: Option<String>,
        #[serde(default)]
        items: Vec<String>,
        #[serde(default)]
        speed: Option<f32>,
        #[serde(default)]
        duration_ms: Option<u64>,
    },
    StatusIndicator {
        id: String,
        #[serde(default)]
        show: Option<bool>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        icon: IconKind,
        #[serde(default)]
        accent: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallTarget {
    pub deck_idx: usize,
    pub layer_idx: usize,
    pub key_idx: usize,
}

struct RunningPlugin {
    _child: Child,
    stdin: ChildStdin,
}

impl Drop for RunningPlugin {
    fn drop(&mut self) {
        if let Ok(line) = serde_json::to_string(&HostMsg::Goodbye) {
            let _ = writeln!(self.stdin, "{line}");
            let _ = self.stdin.flush();
        }
    }
}

struct PluginSlot {
    config: PluginConfig,
    process: Mutex<Option<RunningPlugin>>,
}

pub struct PluginManager {
    slots: HashMap<String, Arc<PluginSlot>>,
    pending: Arc<Mutex<HashMap<u64, CallTarget>>>,
    next_id: AtomicU64,
    handle: Arc<OnceLock<AppHandle<DeckState>>>,
    app_config: serde_json::Value,
}

impl PluginManager {
    pub fn new(configs: Vec<PluginConfig>, app_config: serde_json::Value) -> Self {
        let mut slots = HashMap::new();
        for cfg in configs {
            slots.insert(
                cfg.id.clone(),
                Arc::new(PluginSlot {
                    config: cfg,
                    process: Mutex::new(None),
                }),
            );
        }
        Self {
            slots,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            handle: Arc::new(OnceLock::new()),
            app_config,
        }
    }

    pub fn attach(&self, handle: AppHandle<DeckState>) {
        let _ = self.handle.set(handle);
        let slots: Vec<Arc<PluginSlot>> = self.slots.values().cloned().collect();
        let pending = Arc::clone(&self.pending);
        let handle_ref = Arc::clone(&self.handle);
        let app_config = self.app_config.clone();
        std::thread::spawn(move || {
            for slot in slots {
                let Ok(mut guard) = slot.process.lock() else {
                    continue;
                };
                if guard.is_none()
                    && let Ok(proc) = Self::spawn_plugin_with(
                        &slot.config,
                        Arc::clone(&slot),
                        Arc::clone(&pending),
                        Arc::clone(&handle_ref),
                        &app_config,
                    )
                {
                    *guard = Some(proc);
                }
            }
        });
    }

    /// Shows the loading indicator on `key_idx` only if `seq` is still active after 350 ms.
    pub fn schedule_loading(&self, seq: u64, key_idx: usize) {
        let Some(handle) = self.handle.get().cloned() else {
            return;
        };
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(350));
            handle.update(move |s, _| s.show_loading_if_seq(seq, key_idx));
        });
    }

    pub fn call(
        &self,
        plugin_id: &str,
        command: &str,
        active: bool,
        params: &serde_json::Value,
        target: CallTarget,
    ) -> Result<(), String> {
        let slot = self
            .slots
            .get(plugin_id)
            .ok_or_else(|| format!("unknown plugin '{plugin_id}'"))?;

        let mut guard = slot
            .process
            .lock()
            .map_err(|_| "plugin lock poisoned".to_string())?;

        if guard.is_none() {
            *guard = Some(self.spawn_plugin(&slot.config, Arc::clone(slot))?);
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut map) = self.pending.lock() {
            map.insert(id, target);
        }

        let msg = HostMsg::Call {
            id,
            command,
            active,
            params,
        };
        let line = serde_json::to_string(&msg).map_err(|e| e.to_string())?;

        if let Some(proc) = guard.as_mut() {
            if let Err(err) = writeln!(proc.stdin, "{line}").and_then(|_| proc.stdin.flush()) {
                *guard = None;
                if let Ok(mut map) = self.pending.lock() {
                    map.remove(&id);
                }
                return Err(format!("write to plugin '{plugin_id}': {err}"));
            }
        }

        Ok(())
    }

    fn resolve_plugin_relative(raw: &str) -> Option<std::path::PathBuf> {
        let expanded = crate::config::expand_tilde(raw);
        if expanded.is_absolute() {
            return expanded.exists().then_some(expanded);
        }
        let stripped = raw
            .strip_prefix("plugins/")
            .or_else(|| raw.strip_prefix("plugins\\"))
            .unwrap_or(raw);
        if let Some(dir) = crate::config::plugins_dir() {
            let in_plugins = dir.join(stripped);
            if in_plugins.exists() {
                return Some(in_plugins);
            }
        }
        let in_cwd = std::path::PathBuf::from(raw);
        if in_cwd.exists() {
            return Some(in_cwd);
        }
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let in_exe = dir.join(raw);
            if in_exe.exists() {
                return Some(in_exe);
            }
        }
        None
    }

    fn resolve_command(command: &str) -> std::path::PathBuf {
        let expanded = crate::config::expand_tilde(command);
        if expanded.is_absolute() {
            return expanded;
        }
        let path = std::path::Path::new(command);
        if path.components().count() == 1
            && let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let sibling = dir.join(command);
            if sibling.exists() {
                return sibling;
            }
            #[cfg(target_os = "windows")]
            {
                let sibling_exe = dir.join(format!("{command}.exe"));
                if sibling_exe.exists() {
                    return sibling_exe;
                }
            }
        }
        Self::resolve_plugin_relative(command).unwrap_or(expanded)
    }

    fn resolve_arg(arg: &str) -> String {
        if arg.starts_with('-') {
            return arg.to_string();
        }
        let looks_like_plugin_file = arg.starts_with("plugins/")
            || arg.starts_with("plugins\\")
            || arg.starts_with("~/")
            || arg.starts_with("~\\")
            || arg.ends_with(".py")
            || arg.ends_with(".ps1")
            || arg.ends_with(".sh");
        if looks_like_plugin_file
            && let Some(resolved) = Self::resolve_plugin_relative(arg)
        {
            return resolved.to_string_lossy().into_owned();
        }
        arg.to_string()
    }

    fn spawn_plugin(
        &self,
        cfg: &PluginConfig,
        slot: Arc<PluginSlot>,
    ) -> Result<RunningPlugin, String> {
        Self::spawn_plugin_with(
            cfg,
            slot,
            Arc::clone(&self.pending),
            Arc::clone(&self.handle),
            &self.app_config,
        )
    }

    fn spawn_plugin_with(
        cfg: &PluginConfig,
        slot: Arc<PluginSlot>,
        pending: Arc<Mutex<HashMap<u64, CallTarget>>>,
        handle: Arc<OnceLock<AppHandle<DeckState>>>,
        app_config: &serde_json::Value,
    ) -> Result<RunningPlugin, String> {
        let mut cmd = Command::new(Self::resolve_command(&cfg.command));
        let resolved_args: Vec<String> = cfg.args.iter().map(|a| Self::resolve_arg(a)).collect();
        cmd.args(&resolved_args)
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("spawn plugin '{}': {e}", cfg.id))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("plugin '{}' missing stdin", cfg.id))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("plugin '{}' missing stdout", cfg.id))?;

        let hello = serde_json::to_string(&HostMsg::Hello {
            protocol: PROTOCOL_VERSION,
            config: app_config,
            plugin_config: &cfg.config,
        })
        .map_err(|e| e.to_string())?;
        writeln!(stdin, "{hello}").map_err(|e| format!("hello '{}': {e}", cfg.id))?;
        stdin
            .flush()
            .map_err(|e| format!("hello flush '{}': {e}", cfg.id))?;

        let plugin_id = cfg.id.clone();

        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(msg) = serde_json::from_str::<PluginMsg>(trimmed) else {
                    continue;
                };

                match msg {
                    PluginMsg::Hello { .. } => {}
                    PluginMsg::Result {
                        id,
                        ok,
                        active,
                        label,
                        status,
                        error,
                        decks,
                        open_deck,
                    } => {
                        let target = pending.lock().ok().and_then(|mut m| m.remove(&id));
                        if let Some(target) = target
                            && let Some(h) = handle.get()
                        {
                            h.update(move |s, _| {
                                s.apply_plugin_result(
                                    target, ok, active, label, status, error, decks, open_deck,
                                );
                            });
                        }
                    }
                    PluginMsg::Event {
                        deck,
                        layer,
                        key,
                        active,
                        label,
                        icon,
                        status,
                    } => {
                        if let Some(h) = handle.get() {
                            h.update(move |s, _| {
                                s.apply_plugin_event(&deck, layer, key, active, label, icon, status);
                            });
                        }
                    }
                    PluginMsg::UpsertDecks { decks, open_deck } => {
                        if let Some(h) = handle.get() {
                            h.update(move |s, _| {
                                s.apply_plugin_decks(decks, open_deck);
                            });
                        }
                    }
                    PluginMsg::Toast {
                        title,
                        detail,
                        icon,
                        accent,
                        error,
                    } => {
                        if let Some(h) = handle.get() {
                            let color = parse_accent(accent.as_deref().unwrap_or("cyan"));
                            h.update(move |s, _| {
                                s.show_plugin_toast(title, detail, icon, color, error);
                            });
                        }
                    }
                    PluginMsg::NewsStrip {
                        show,
                        title,
                        accent,
                        items,
                        speed,
                        duration_ms,
                    } => {
                        if let Some(h) = handle.get() {
                            let visible = show.unwrap_or(!items.is_empty());
                            let color = parse_accent(accent.as_deref().unwrap_or("amber"));
                            h.update(move |s, _| {
                                s.show_news_strip(visible, title, color, items, speed, duration_ms);
                            });
                        }
                    }
                    PluginMsg::StatusIndicator {
                        id,
                        show,
                        label,
                        icon,
                        accent,
                    } => {
                        if let Some(h) = handle.get() {
                            let visible = show.unwrap_or(true);
                            let color = parse_accent(accent.as_deref().unwrap_or("amber"));
                            h.update(move |s, _| {
                                s.show_status_indicator(id, visible, label, icon, color);
                            });
                        }
                    }
                }
            }

            // Plugin stdout closed: clear process handle so next call can respawn it.
            if let Ok(mut guard) = slot.process.lock() {
                *guard = None;
            }
            let _ = plugin_id;
        });

        Ok(RunningPlugin {
            _child: child,
            stdin,
        })
    }
}
