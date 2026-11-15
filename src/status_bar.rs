use crate::config::IconKind;
use crate::icons::{icon_el, prefetch, register_repaint_hook};
use rune::tok;
use rune::{
    Align, Color, ConfiguredOverlay, Cx, DisplayTarget, OverlayHandle, OverlayOptions,
    OverlaySpec, ScreenEdge, oklch, oklcha, row, spacer, text,
};
use std::sync::{Arc, Mutex};

pub const STATUS_BAR_SIZE: (f32, f32) = (540.0, 38.0);

#[derive(Clone, Debug, PartialEq)]
pub struct StatusIndicator {
    pub id: String,
    pub label: String,
    pub icon: IconKind,
    pub accent: Color,
}

#[derive(Clone, Debug, Default)]
pub struct StatusBarState {
    pub items: Vec<StatusIndicator>,
}

pub fn spec() -> ConfiguredOverlay<StatusBarState> {
    OverlaySpec {
        edge: ScreenEdge::Top,
        margin: 14.0,
        align: Align::End,
        ..OverlaySpec::new("vdeck-status-bar", STATUS_BAR_SIZE, view)
    }
    .options(OverlayOptions {
        display: DisplayTarget::Primary,
        prewarm: true,
        ..Default::default()
    })
}

pub fn view(s: &StatusBarState, _: &mut Cx<StatusBarState>) -> rune::El {
    let mut bar = row()
        .key("status-bar")
        .w_full()
        .h_full()
        .center()
        .gap(6.0)
        .child(spacer());

    for item in &s.items {
        let pill = row()
            .key(format!("status-pill-{}", item.id))
            .h(26.0)
            .px(10.0)
            .gap(6.0)
            .radius(8.0)
            .center()
            .gradient(oklch(0.16, 0.008, 260.0), oklch(0.13, 0.007, 260.0))
            .border_gradient(1.0, item.accent.alpha(0.50), oklcha(1.0, 0.0, 0.0, 0.06))
            .shadow(4.0, 12.0, oklcha(0.0, 0.0, 0.0, 0.45))
            .child(icon_el(&item.icon, 14.0, item.accent))
            .child(
                text(&item.label)
                    .font(tok::TextStyle::new(11.0, 600.0).tracking(0.3))
                    .color(oklcha(1.0, 0.0, 0.0, 0.92)),
            );
        bar = bar.child(pill);
    }

    bar
}

#[derive(Clone)]
pub struct StatusBarController {
    handle: OverlayHandle<StatusBarState>,
    items: Arc<Mutex<Vec<StatusIndicator>>>,
}

impl StatusBarController {
    pub fn new(handle: OverlayHandle<StatusBarState>) -> Self {
        let repaint_handle = handle.clone();
        register_repaint_hook(move || {
            repaint_handle.update(|_, _| {});
        });
        Self {
            handle,
            items: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn update_indicator(
        &self,
        id: String,
        show: bool,
        label: Option<String>,
        icon: IconKind,
        accent: Color,
    ) {
        if show {
            prefetch(&icon);
        }
        let snapshot = {
            let mut guard = self.items.lock().unwrap_or_else(|e| e.into_inner());
            if show {
                let text = label.filter(|l| !l.is_empty()).unwrap_or_else(|| id.to_uppercase());
                if let Some(existing) = guard.iter_mut().find(|item| item.id == id) {
                    existing.label = text;
                    existing.icon = icon;
                    existing.accent = accent;
                } else {
                    guard.push(StatusIndicator {
                        id,
                        label: text,
                        icon,
                        accent,
                    });
                }
            } else {
                guard.retain(|item| item.id != id);
            }
            guard.clone()
        };

        let has_items = !snapshot.is_empty();
        self.handle.update(move |s, _| {
            s.items = snapshot;
        });
        if has_items {
            self.handle.show();
        } else {
            self.handle.hide();
        }
    }
}
