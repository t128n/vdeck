use rune::color::hex;
use rune::tok;
use rune::{
    Align, Color, ConfiguredOverlay, Cx, DisplayTarget, OverlayHandle, OverlayOptions,
    OverlaySpec, ScreenEdge, Spring, Tx, oklch, oklcha, row, text,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub const STRIP_SIZE: (f32, f32) = (1880.0, 34.0);
const DEFAULT_SPEED: f32 = 28.0;

#[derive(Clone, Debug, PartialEq)]
pub struct NewsSpan {
    pub text: String,
    pub color: Color,
}

#[derive(Clone, Debug)]
pub struct NewsStripState {
    pub title: String,
    pub accent: Color,
    pub spans: Vec<NewsSpan>,
    pub loop_width: f32,
    pub scroll_x: f32,
    pub speed: f32,
    pub visible: bool,
}

impl Default for NewsStripState {
    fn default() -> Self {
        Self {
            title: "NEWS".to_string(),
            accent: oklch(0.72, 0.16, 50.0),
            spans: Vec::new(),
            loop_width: 400.0,
            scroll_x: 0.0,
            speed: DEFAULT_SPEED,
            visible: false,
        }
    }
}

fn ansi_sgr_color(code: u8, default_color: Color) -> Option<Color> {
    match code {
        0 | 39 => Some(default_color),
        30 => Some(oklcha(1.0, 0.0, 0.0, 0.45)),
        31 => Some(oklch(0.66, 0.21, 25.0)),
        32 => Some(oklch(0.72, 0.17, 150.0)),
        33 => Some(oklch(0.78, 0.16, 80.0)),
        34 => Some(oklch(0.68, 0.18, 240.0)),
        35 => Some(oklch(0.70, 0.17, 295.0)),
        36 => Some(oklch(0.74, 0.15, 200.0)),
        37 => Some(oklcha(1.0, 0.0, 0.0, 0.85)),
        90 => Some(oklcha(1.0, 0.0, 0.0, 0.50)),
        91 => Some(oklch(0.70, 0.22, 25.0)),
        92 => Some(oklch(0.78, 0.19, 150.0)),
        93 => Some(oklch(0.84, 0.16, 82.0)),
        94 => Some(oklch(0.74, 0.17, 240.0)),
        95 => Some(oklch(0.76, 0.17, 300.0)),
        96 => Some(oklch(0.80, 0.14, 200.0)),
        97 => Some(oklcha(1.0, 0.0, 0.0, 0.96)),
        _ => None,
    }
}

/// Parses ANSI SGR escape sequences (`\x1b[...m`) into uppercase colored spans.
pub fn parse_ansi_spans(raw: &str, default_color: Color) -> Vec<NewsSpan> {
    let mut spans: Vec<NewsSpan> = Vec::new();
    let mut current_color = default_color;
    let mut rest = raw;

    while let Some(esc_idx) = rest.find('\x1b') {
        let before = &rest[..esc_idx];
        if !before.is_empty() {
            push_span(&mut spans, before.to_uppercase(), current_color);
        }
        let after_esc = &rest[esc_idx + 1..];
        if let Some(body) = after_esc.strip_prefix('[')
            && let Some(m_idx) = body.find('m')
        {
            let params: Vec<u8> = body[..m_idx]
                .split(';')
                .filter_map(|p| if p.is_empty() { Some(0) } else { p.parse().ok() })
                .collect();
            let mut i = 0;
            if params.is_empty() {
                current_color = default_color;
            }
            while i < params.len() {
                if params[i] == 38 && i + 4 < params.len() && params[i + 1] == 2 {
                    let spec = format!("#{:02x}{:02x}{:02x}", params[i + 2], params[i + 3], params[i + 4]);
                    if let Some(c) = hex(&spec) {
                        current_color = c;
                    }
                    i += 5;
                    continue;
                }
                if let Some(c) = ansi_sgr_color(params[i], default_color) {
                    current_color = c;
                }
                i += 1;
            }
            rest = &body[m_idx + 1..];
        } else {
            rest = after_esc;
        }
    }

    if !rest.is_empty() {
        push_span(&mut spans, rest.to_uppercase(), current_color);
    }
    spans
}

fn push_span(spans: &mut Vec<NewsSpan>, text: String, color: Color) {
    if let Some(last) = spans.last_mut()
        && last.color == color
    {
        last.text.push_str(&text);
    } else {
        spans.push(NewsSpan { text, color });
    }
}

fn poll_marquee(s: &mut NewsStripState, dt: f64, tx: &mut Tx) -> bool {
    if !s.visible || s.spans.is_empty() || s.loop_width <= 1.0 {
        return false;
    }
    let step_dt = (dt as f32).clamp(0.0, 0.05);
    s.scroll_x += s.speed * step_dt;
    if s.scroll_x >= s.loop_width {
        s.scroll_x %= s.loop_width;
    }
    tx.wake_after(Duration::ZERO);
    true
}

pub fn spec() -> ConfiguredOverlay<NewsStripState> {
    OverlaySpec {
        edge: ScreenEdge::Top,
        margin: 10.0,
        align: Align::Center,
        ..OverlaySpec::new("vdeck-news-strip", STRIP_SIZE, view)
    }
    .options(OverlayOptions {
        display: DisplayTarget::UnderPointer,
        prewarm: true,
        ..Default::default()
    })
    .poll(poll_marquee)
}

fn pass_row(key: &'static str, spans: &[NewsSpan]) -> rune::El {
    let mut r = row().key(key);
    for (idx, span) in spans.iter().enumerate() {
        r = r.child(
            text(&span.text)
                .key(format!("{key}-{idx}"))
                .font(
                    tok::TextStyle::new(11.5, 500.0)
                        .family(tok::Family::SystemMono)
                        .tracking(0.35),
                )
                .color(span.color),
        );
    }
    r
}

pub fn view(s: &NewsStripState, _: &mut Cx<NewsStripState>) -> rune::El {
    let badge = row()
        .key("news-badge")
        .h(20.0)
        .px(8.0)
        .radius(4.0)
        .center()
        .fill(s.accent.alpha(0.22))
        .border_gradient(1.0, s.accent.alpha(0.55), s.accent.alpha(0.20))
        .child(
            text(&s.title)
                .font(
                    tok::TextStyle::new(10.5, 700.0)
                        .family(tok::Family::SystemMono)
                        .tracking(0.6),
                )
                .color(s.accent),
        );

    let pass_0 = pass_row("news-pass-0", &s.spans).on_layout(
        |state: &mut NewsStripState, rect| {
            if rect[2] > 1.0 && (state.loop_width - rect[2]).abs() > 0.5 {
                state.loop_width = rect[2];
            }
        },
    );
    let pass_1 = pass_row("news-pass-1", &s.spans);
    let pass_2 = pass_row("news-pass-2", &s.spans);

    let track_w = STRIP_SIZE.0 - 88.0;
    let marquee_track = row()
        .key("news-track")
        .w(track_w)
        .h(22.0)
        .items_center()
        .justify_start()
        .clip_fade_linear(20.0, 0.0, 20.0, 0.0)
        .child(
            row()
                .key("news-scroller")
                .w(12000.0)
                .items_center()
                .justify_start()
                .offset(-s.scroll_x, 0.0)
                .child(pass_0)
                .child(pass_1)
                .child(pass_2),
        );

    let bar = row()
        .key("news-bar")
        .w(STRIP_SIZE.0)
        .h(30.0)
        .px(10.0)
        .gap(10.0)
        .radius(6.0)
        .center()
        .offset(0.0, if s.visible { 0.0 } else { -20.0 })
        .offset_motion(Spring::new(0.24, 0.12))
        .opacity(if s.visible { 1.0 } else { 0.0 })
        .opacity_motion(Spring::new(0.16, 0.0))
        .gradient(oklch(0.15, 0.008, 260.0), oklch(0.125, 0.007, 260.0))
        .border_gradient(1.0, s.accent.alpha(0.35), oklcha(1.0, 0.0, 0.0, 0.06))
        .shadow(6.0, 16.0, oklcha(0.0, 0.0, 0.0, 0.50))
        .child(badge)
        .child(marquee_track);

    row().key("news-root").w_full().h_full().center().child(bar)
}

#[derive(Clone)]
pub struct NewsStripController {
    handle: OverlayHandle<NewsStripState>,
    seq: Arc<AtomicU64>,
}

impl NewsStripController {
    pub fn new(handle: OverlayHandle<NewsStripState>) -> Self {
        Self {
            handle,
            seq: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn update_strip(
        &self,
        show: bool,
        title: Option<String>,
        accent: Color,
        items: Vec<String>,
        speed: Option<f32>,
        duration_ms: Option<u64>,
    ) {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        if !show || items.is_empty() {
            self.handle.update(|s, _| s.visible = false);
            let seq_ref = Arc::clone(&self.seq);
            let handle = self.handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                if seq_ref.load(Ordering::Relaxed) == seq {
                    handle.hide();
                }
            });
            return;
        }

        let default_color = oklcha(1.0, 0.0, 0.0, 0.92);
        let raw = format!("{}\x1b[0m   \x1b[90m|\x1b[0m   ", items.join("\x1b[0m   \x1b[90m|\x1b[0m   "));
        let spans = parse_ansi_spans(&raw, default_color);
        let est_chars: usize = spans.iter().map(|s| s.text.chars().count()).sum();
        let loop_width = (est_chars as f32 * 7.2).max(320.0);
        let speed = speed.unwrap_or(DEFAULT_SPEED).clamp(10.0, 200.0);
        let title = title.unwrap_or_else(|| "NEWS".to_string()).to_uppercase();

        self.handle.update(move |s, _| {
            s.title = title;
            s.accent = accent;
            s.spans = spans;
            s.loop_width = loop_width;
            s.scroll_x = 0.0;
            s.speed = speed;
            s.visible = true;
        });
        self.handle.show();

        if let Some(ms) = duration_ms {
            let seq_ref = Arc::clone(&self.seq);
            let handle = self.handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(ms));
                if seq_ref.load(Ordering::Relaxed) != seq {
                    return;
                }
                handle.update(|s, _| s.visible = false);
                std::thread::sleep(Duration::from_millis(200));
                if seq_ref.load(Ordering::Relaxed) == seq {
                    handle.hide();
                }
            });
        }
    }
}
