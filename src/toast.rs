use crate::config::IconKind;
use crate::icons::{icon_el, register_repaint_hook};
use rune::tok;
use rune::{
    Align, Color, ConfiguredOverlay, Cx, DisplayTarget, OverlayHandle, OverlayOptions,
    OverlaySpec, ScreenEdge, Spring, el, oklch, oklcha, row, spacer, text,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub const TOAST_SIZE: (f32, f32) = (280.0, 48.0);

#[derive(Clone, Debug)]
pub struct ToastState {
    pub title: String,
    pub detail: Option<String>,
    pub kind: IconKind,
    pub accent: Color,
    pub is_error: bool,
    pub visible: bool,
}

impl Default for ToastState {
    fn default() -> Self {
        Self {
            title: String::new(),
            detail: None,
            kind: IconKind::Circle,
            accent: oklch(0.72, 0.16, 200.0),
            is_error: false,
            visible: false,
        }
    }
}

pub fn spec() -> ConfiguredOverlay<ToastState> {
    OverlaySpec {
        edge: ScreenEdge::Top,
        margin: 16.0,
        align: Align::Center,
        ..OverlaySpec::new("vdeck-toast", TOAST_SIZE, view)
    }
    .options(OverlayOptions {
        display: DisplayTarget::UnderPointer,
        prewarm: true,
        ..Default::default()
    })
}

pub fn view(s: &ToastState, _: &mut Cx<ToastState>) -> rune::El {
    let icon_color = if s.is_error {
        oklch(0.72, 0.18, 25.0)
    } else {
        s.accent
    };
    let border_top = if s.is_error {
        oklcha(0.72, 0.18, 25.0, 0.55)
    } else {
        s.accent.alpha(0.45)
    };

    let mut pill = row()
        .key("toast-pill")
        .h(38.0)
        .px(14.0)
        .gap(10.0)
        .radius(11.0)
        .center()
        .offset(0.0, if s.visible { 0.0 } else { -18.0 })
        .offset_motion(Spring::new(0.24, 0.12))
        .opacity(if s.visible { 1.0 } else { 0.0 })
        .opacity_motion(Spring::new(0.16, 0.0))
        .gradient(oklch(0.16, 0.008, 260.0), oklch(0.13, 0.007, 260.0))
        .border_gradient(1.0, border_top, oklcha(1.0, 0.0, 0.0, 0.06))
        .shadow(6.0, 16.0, oklcha(0.0, 0.0, 0.0, 0.50))
        .child(icon_el(&s.kind, 18.0, icon_color))
        .child(
            text(&s.title)
                .font(tok::TextStyle::new(12.5, 560.0).tracking(-0.1))
                .color(oklcha(1.0, 0.0, 0.0, 0.92)),
        );

    if let Some(detail) = &s.detail
        && !detail.is_empty()
    {
        pill = pill
            .child(
                el().key("toast-dot")
                    .size(3.0, 3.0)
                    .radius(1.5)
                    .fill(oklcha(1.0, 0.0, 0.0, 0.25)),
            )
            .child(
                text(detail)
                    .font(tok::TextStyle::new(11.5, 450.0))
                    .color(if s.is_error {
                        oklch(0.74, 0.16, 25.0)
                    } else {
                        oklcha(1.0, 0.0, 0.0, 0.60)
                    }),
            );
    }

    row()
        .key("toast-root")
        .w_full()
        .h_full()
        .center()
        .child(spacer())
        .child(pill)
        .child(spacer())
}

#[derive(Clone)]
pub struct ToastController {
    handle: OverlayHandle<ToastState>,
    seq: Arc<AtomicU64>,
}

impl ToastController {
    pub fn new(handle: OverlayHandle<ToastState>) -> Self {
        let repaint_handle = handle.clone();
        register_repaint_hook(move || {
            repaint_handle.update(|_, _| {});
        });
        Self {
            handle,
            seq: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn show(
        &self,
        title: String,
        detail: Option<String>,
        kind: IconKind,
        accent: Color,
        is_error: bool,
    ) {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        self.handle.update(move |s, _| {
            s.title = title;
            s.detail = detail;
            s.kind = kind;
            s.accent = accent;
            s.is_error = is_error;
            s.visible = true;
        });
        self.handle.show();

        let seq_ref = Arc::clone(&self.seq);
        let handle = self.handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1450));
            if seq_ref.load(Ordering::Relaxed) != seq {
                return;
            }
            handle.update(|s, _| s.visible = false);
            std::thread::sleep(Duration::from_millis(220));
            if seq_ref.load(Ordering::Relaxed) == seq {
                handle.hide();
            }
        });
    }
}
