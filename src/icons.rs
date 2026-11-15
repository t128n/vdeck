use crate::config::IconKind;
use rune::shape::LineCap;
use rune::{Color, El, Icon, Shape, canvas, icon};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

pub fn icon_shape(kind: &IconKind, color: Color) -> Shape {
    let w = 1.65;
    let shape = match kind {
        IconKind::Mic => Shape::new()
            .stroke_with(
                vec![[9.5, 4.0], [14.5, 4.0], [14.5, 12.0], [9.5, 12.0]],
                true,
                w,
                LineCap::Round,
                color,
            )
            .stroke(
                vec![
                    [6.5, 11.5],
                    [7.5, 15.0],
                    [12.0, 16.5],
                    [16.5, 15.0],
                    [17.5, 11.5],
                ],
                w,
                color,
            )
            .line([12.0, 16.5], [12.0, 20.0], w, LineCap::Round, color)
            .line([9.0, 20.0], [15.0, 20.0], w, LineCap::Round, color),
        IconKind::Camera => Shape::new()
            .stroke_with(
                vec![[3.5, 7.0], [15.5, 7.0], [15.5, 17.0], [3.5, 17.0]],
                true,
                w,
                LineCap::Round,
                color,
            )
            .stroke_with(
                vec![[15.5, 10.5], [20.5, 8.0], [20.5, 16.0], [15.5, 13.5]],
                true,
                w,
                LineCap::Round,
                color,
            ),
        IconKind::Record => Shape::new()
            .stroke_with(
                vec![
                    [12.0, 4.0],
                    [17.6, 6.4],
                    [20.0, 12.0],
                    [17.6, 17.6],
                    [12.0, 20.0],
                    [6.4, 17.6],
                    [4.0, 12.0],
                    [6.4, 6.4],
                ],
                true,
                w,
                LineCap::Round,
                color,
            )
            .fill(
                vec![
                    [12.0, 8.5],
                    [14.5, 9.5],
                    [15.5, 12.0],
                    [14.5, 14.5],
                    [12.0, 15.5],
                    [9.5, 14.5],
                    [8.5, 12.0],
                    [9.5, 9.5],
                ],
                color,
            ),
        IconKind::Layers => Shape::new()
            .stroke_with(
                vec![[12.0, 4.0], [20.0, 8.5], [12.0, 13.0], [4.0, 8.5]],
                true,
                w,
                LineCap::Round,
                color,
            )
            .stroke(vec![[4.0, 12.5], [12.0, 17.0], [20.0, 12.5]], w, color)
            .stroke(vec![[4.0, 16.5], [12.0, 21.0], [20.0, 16.5]], w, color),
        IconKind::Scissors => Shape::new()
            .stroke_with(
                vec![[5.5, 16.5], [8.0, 15.0], [9.5, 17.5], [7.0, 19.0]],
                true,
                w,
                LineCap::Round,
                color,
            )
            .stroke_with(
                vec![[5.5, 7.5], [8.0, 9.0], [9.5, 6.5], [7.0, 5.0]],
                true,
                w,
                LineCap::Round,
                color,
            )
            .line([9.0, 8.5], [19.0, 16.5], w, LineCap::Round, color)
            .line([9.0, 15.5], [19.0, 7.5], w, LineCap::Round, color),
        IconKind::Play => Shape::new().fill(vec![[7.0, 5.0], [19.0, 12.0], [7.0, 19.0]], color),
        IconKind::Sun => Shape::new()
            .stroke_with(
                vec![
                    [12.0, 8.0],
                    [14.8, 9.2],
                    [16.0, 12.0],
                    [14.8, 14.8],
                    [12.0, 16.0],
                    [9.2, 14.8],
                    [8.0, 12.0],
                    [9.2, 9.2],
                ],
                true,
                w,
                LineCap::Round,
                color,
            )
            .line([12.0, 3.5], [12.0, 5.5], w, LineCap::Round, color)
            .line([12.0, 18.5], [12.0, 20.5], w, LineCap::Round, color)
            .line([3.5, 12.0], [5.5, 12.0], w, LineCap::Round, color)
            .line([18.5, 12.0], [20.5, 12.0], w, LineCap::Round, color)
            .line([6.0, 6.0], [7.5, 7.5], w, LineCap::Round, color)
            .line([16.5, 16.5], [18.0, 18.0], w, LineCap::Round, color)
            .line([18.0, 6.0], [16.5, 7.5], w, LineCap::Round, color)
            .line([7.5, 16.5], [6.0, 18.0], w, LineCap::Round, color),
        IconKind::Terminal => Shape::new()
            .stroke_with(
                vec![[3.5, 5.0], [20.5, 5.0], [20.5, 19.0], [3.5, 19.0]],
                true,
                w,
                LineCap::Round,
                color,
            )
            .stroke(vec![[7.5, 9.5], [10.5, 12.0], [7.5, 14.5]], w, color)
            .line([12.5, 14.5], [16.5, 14.5], w, LineCap::Round, color),
        IconKind::Lock => Shape::new()
            .stroke_with(
                vec![[5.5, 11.0], [18.5, 11.0], [18.5, 19.5], [5.5, 19.5]],
                true,
                w,
                LineCap::Round,
                color,
            )
            .stroke(
                vec![
                    [8.5, 11.0],
                    [8.5, 7.0],
                    [12.0, 5.0],
                    [15.5, 7.0],
                    [15.5, 11.0],
                ],
                w,
                color,
            ),
        IconKind::Folder => Shape::new().stroke_with(
            vec![
                [3.5, 6.0],
                [9.5, 6.0],
                [11.5, 8.5],
                [20.5, 8.5],
                [20.5, 18.5],
                [3.5, 18.5],
            ],
            true,
            w,
            LineCap::Round,
            color,
        ),
        IconKind::Back => Shape::new()
            .line([19.0, 12.0], [6.0, 12.0], w, LineCap::Round, color)
            .stroke(vec![[11.0, 7.0], [6.0, 12.0], [11.0, 17.0]], w, color),
        IconKind::ArrowUp => Shape::new()
            .line([12.0, 19.0], [12.0, 5.5], w, LineCap::Round, color)
            .stroke(vec![[7.0, 10.5], [12.0, 5.5], [17.0, 10.5]], w, color),
        IconKind::ArrowDown => Shape::new()
            .line([12.0, 5.0], [12.0, 18.5], w, LineCap::Round, color)
            .stroke(vec![[7.0, 13.5], [12.0, 18.5], [17.0, 13.5]], w, color),
        IconKind::Volume => Shape::new()
            .stroke_with(
                vec![
                    [4.5, 9.5],
                    [8.5, 9.5],
                    [13.5, 5.5],
                    [13.5, 18.5],
                    [8.5, 14.5],
                    [4.5, 14.5],
                ],
                true,
                w,
                LineCap::Round,
                color,
            )
            .stroke(vec![[17.0, 9.0], [18.5, 12.0], [17.0, 15.0]], w, color),
        IconKind::Circle | IconKind::Custom(_) => Shape::new().stroke_with(
            vec![
                [12.0, 7.0],
                [15.5, 8.5],
                [17.0, 12.0],
                [15.5, 15.5],
                [12.0, 17.0],
                [8.5, 15.5],
                [7.0, 12.0],
                [8.5, 8.5],
            ],
            true,
            w,
            LineCap::Round,
            color,
        ),
    };
    shape.view_box(24.0, 24.0)
}

/// Renders an `IconKind` into a Rune `El` at `size` points, resolving custom SVG paths or Iconify shortcuts.
pub fn icon_el(kind: &IconKind, size: f32, color: Color) -> El {
    if let Some(spec) = kind.custom_spec()
        && let Some(svg_icon) = resolve_custom_icon(spec)
    {
        return icon(svg_icon, size).color(color);
    }
    canvas(icon_shape(kind, color)).size(size, size)
}

/// Starts resolving/fetching a custom icon in the background if it is not already cached.
pub fn prefetch(kind: &IconKind) {
    if let Some(spec) = kind.custom_spec() {
        let _ = resolve_custom_icon(spec);
    }
}

type RepaintHook = Arc<dyn Fn() + Send + Sync>;

static REPAINT_HOOKS: OnceLock<Mutex<Vec<RepaintHook>>> = OnceLock::new();

/// Registers a callback invoked when an asynchronous icon fetch finishes.
pub fn register_repaint_hook<F: Fn() + Send + Sync + 'static>(hook: F) {
    let hooks = REPAINT_HOOKS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = hooks.lock() {
        guard.push(Arc::new(hook));
    }
}

fn notify_repaint() {
    let Some(hooks) = REPAINT_HOOKS.get() else {
        return;
    };
    let snapshot: Vec<RepaintHook> = match hooks.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return,
    };
    for hook in snapshot {
        hook();
    }
}

#[derive(Clone, Copy)]
enum CachedIcon {
    Pending,
    Ready(&'static Icon),
    Failed,
}

static ICON_CACHE: OnceLock<RwLock<HashMap<String, CachedIcon>>> = OnceLock::new();

fn icon_cache() -> &'static RwLock<HashMap<String, CachedIcon>> {
    ICON_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

#[derive(Debug, PartialEq, Eq)]
enum IconSource {
    LocalPath(PathBuf),
    Iconify { spec: String, url: String },
}

fn iconify_url(spec: &str) -> String {
    if let Some((prefix, name)) = spec.split_once(':') {
        format!("https://api.iconify.design/{prefix}/{name}.svg")
    } else {
        format!("https://api.iconify.design/{spec}.svg")
    }
}

fn parse_icon_source(spec: &str) -> IconSource {
    let trimmed = spec.trim();
    if let Some(rest) = trimmed.strip_prefix("iconify:") {
        let clean = rest.trim();
        return IconSource::Iconify {
            spec: clean.to_string(),
            url: iconify_url(clean),
        };
    }

    let looks_like_path = trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.starts_with('.')
        || trimmed.starts_with('~')
        || trimmed.to_ascii_lowercase().ends_with(".svg");

    if !looks_like_path
        && trimmed.contains(':')
        && !trimmed.contains("://")
        && !trimmed.chars().any(char::is_whitespace)
    {
        return IconSource::Iconify {
            spec: trimmed.to_string(),
            url: iconify_url(trimmed),
        };
    }

    IconSource::LocalPath(expand_local_path(trimmed))
}

fn expand_local_path(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
        && let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
    {
        return PathBuf::from(home).join(rest);
    }

    let path = PathBuf::from(raw);
    if path.is_absolute() || path.exists() {
        return path;
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join(&path);
        if candidate.exists() {
            return candidate;
        }
    }

    path
}

fn iconify_cache_path(spec: &str) -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("vdeck")
        .join("icons");

    let sanitized: String = spec
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '-',
        })
        .collect();
    base.join(format!("{sanitized}.svg"))
}

fn resolve_custom_icon(spec: &str) -> Option<&'static Icon> {
    {
        let guard = icon_cache().read().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = guard.get(spec) {
            return match entry {
                CachedIcon::Ready(icon) => Some(*icon),
                CachedIcon::Pending | CachedIcon::Failed => None,
            };
        }
    }

    match parse_icon_source(spec) {
        IconSource::LocalPath(path) => {
            let loaded = load_svg_from_file(spec, &path);
            let mut guard = icon_cache().write().unwrap_or_else(|e| e.into_inner());
            if let Some(CachedIcon::Ready(icon)) = guard.get(spec) {
                return Some(*icon);
            }
            match loaded {
                Some(icon) => {
                    guard.insert(spec.to_string(), CachedIcon::Ready(icon));
                    Some(icon)
                }
                None => {
                    guard.insert(spec.to_string(), CachedIcon::Failed);
                    None
                }
            }
        }
        IconSource::Iconify {
            spec: iconify_spec,
            url,
        } => {
            let cache_path = iconify_cache_path(&iconify_spec);
            if cache_path.exists()
                && let Some(icon) = load_svg_from_file(spec, &cache_path)
            {
                let mut guard = icon_cache().write().unwrap_or_else(|e| e.into_inner());
                guard.insert(spec.to_string(), CachedIcon::Ready(icon));
                return Some(icon);
            }

            {
                let mut guard = icon_cache().write().unwrap_or_else(|e| e.into_inner());
                if let Some(entry) = guard.get(spec) {
                    return match entry {
                        CachedIcon::Ready(icon) => Some(*icon),
                        CachedIcon::Pending | CachedIcon::Failed => None,
                    };
                }
                guard.insert(spec.to_string(), CachedIcon::Pending);
            }

            let key = spec.to_string();
            std::thread::spawn(move || {
                let fetched = fetch_svg(&url).and_then(|svg| {
                    let parsed = parse_svg_to_icon(&key, &svg)?;
                    if let Some(parent) = cache_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&cache_path, svg);
                    Some(parsed)
                });

                {
                    let mut guard = icon_cache().write().unwrap_or_else(|e| e.into_inner());
                    match fetched {
                        Some(icon) => {
                            guard.insert(key, CachedIcon::Ready(icon));
                        }
                        None => {
                            guard.insert(key, CachedIcon::Failed);
                        }
                    }
                }
                if fetched.is_some() {
                    notify_repaint();
                }
            });

            None
        }
    }
}

fn load_svg_from_file(name: &str, path: &Path) -> Option<&'static Icon> {
    let raw = std::fs::read_to_string(path).ok()?;
    parse_svg_to_icon(name, &raw)
}

fn fetch_svg(url: &str) -> Option<String> {
    let mut cmd = Command::new("curl");
    cmd.args(["-fsSL", "--max-time", "5", url]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Parses an SVG document into a leaked `'static rune::Icon`.
pub fn parse_svg_to_icon(name: &str, raw_svg: &str) -> Option<&'static Icon> {
    let svg_start = find_ascii_ci(raw_svg, "<svg")?;
    let trimmed = raw_svg[svg_start..].trim();
    let tag_end_rel = trimmed.find('>')?;
    let svg_open_tag = &trimmed[..=tag_end_rel];
    let normalized_open = normalize_single_quotes(svg_open_tag);

    let (min_x, min_y, view) = parse_view_dimensions(&normalized_open);
    let body = if min_x != 0.0 || min_y != 0.0 {
        if let Some(close_idx) = rfind_ascii_ci(trimmed, "</svg>") {
            let inner = normalize_single_quotes(&trimmed[tag_end_rel + 1..close_idx]);
            format!(
                "{normalized_open}<g transform=\"translate({} {})\">{inner}</g></svg>",
                -min_x, -min_y
            )
        } else {
            normalize_single_quotes(trimmed)
        }
    } else {
        normalize_single_quotes(trimmed)
    };

    if !body.contains("</svg>") && !svg_open_tag.ends_with("/>") {
        return None;
    }

    let leaked_name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let leaked_body: &'static str = Box::leak(body.into_boxed_str());
    Some(Box::leak(Box::new(Icon {
        name: leaked_name,
        view,
        body: leaked_body,
    })))
}

fn find_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    h.windows(n.len()).position(|w| w.eq_ignore_ascii_case(n))
}

fn rfind_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    h.windows(n.len()).rposition(|w| w.eq_ignore_ascii_case(n))
}

fn normalize_single_quotes(svg: &str) -> String {
    if !svg.contains('\'') {
        return svg.to_string();
    }
    let mut out = String::with_capacity(svg.len());
    let mut in_tag = false;
    let mut in_double = false;
    let mut in_single = false;

    for ch in svg.chars() {
        match ch {
            '<' if !in_double && !in_single => {
                in_tag = true;
                out.push(ch);
            }
            '>' if in_tag && !in_double && !in_single => {
                in_tag = false;
                out.push(ch);
            }
            '"' if in_tag && !in_single => {
                in_double = !in_double;
                out.push(ch);
            }
            '\'' if in_tag && !in_double => {
                in_single = !in_single;
                out.push('"');
            }
            _ => out.push(ch),
        }
    }
    out
}

fn extract_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    loop {
        let i = rest.find(name)?;
        let before = rest[..i].chars().last();
        let after = &rest[i + name.len()..];
        if before.is_some_and(char::is_whitespace) && after.starts_with("=\"") {
            let v = &after[2..];
            return Some(&v[..v.find('"')?]);
        }
        rest = &rest[i + name.len()..];
    }
}

fn parse_view_dimensions(svg_open_tag: &str) -> (f32, f32, f32) {
    if let Some(vb) = extract_attr(svg_open_tag, "viewBox") {
        let nums: Vec<f32> = vb
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f32>().ok())
            .collect();
        if nums.len() >= 4 && nums[2] > 0.0 && nums[3] > 0.0 {
            return (nums[0], nums[1], nums[2].max(nums[3]));
        }
    }

    let w = extract_attr(svg_open_tag, "width")
        .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
        .filter(|v| *v > 0.0);
    let h = extract_attr(svg_open_tag, "height")
        .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok())
        .filter(|v| *v > 0.0);

    match (w, h) {
        (Some(w), Some(h)) => (0.0, 0.0, w.max(h)),
        (Some(w), None) => (0.0, 0.0, w),
        (None, Some(h)) => (0.0, 0.0, h),
        (None, None) => (0.0, 0.0, 24.0),
    }
}
