use crate::config::KeyAction;
use crate::deck::{DeckKey, DeckState, numpad_key_for_index};
use crate::icons::icon_el;
use rune::tok::{self, motion};
use rune::{Cx, El, Spring, col, el, oklch, oklcha, row, spacer, text};

pub const WINDOW_SIZE: (f32, f32) = (420.0, 456.0);

fn deck_key_view(idx: usize, key: &DeckKey, is_pressed: bool, is_loading: bool) -> El {
    let active = key.active;
    let is_nav = matches!(key.action, KeyAction::OpenDeck { .. } | KeyAction::Back);
    let is_empty = key.action == KeyAction::None;
    let slot_num = format!("{}", numpad_key_for_index(idx));

    // Linear / Raycast surface ladder (opaque tones so partial-present dirty rects never shift alpha).
    let top_fill = if active {
        oklch(0.21, 0.012, 260.0)
    } else if is_empty {
        oklch(0.145, 0.006, 260.0)
    } else {
        oklch(0.185, 0.008, 260.0)
    };
    let bottom_fill = if active {
        oklch(0.17, 0.010, 260.0)
    } else if is_empty {
        oklch(0.135, 0.006, 260.0)
    } else {
        oklch(0.162, 0.007, 260.0)
    };

    let border_top = if is_pressed || is_loading {
        key.accent.alpha(0.85)
    } else if active {
        key.accent.alpha(0.60)
    } else if is_empty {
        oklcha(1.0, 0.0, 0.0, 0.03)
    } else {
        oklcha(1.0, 0.0, 0.0, 0.09)
    };
    let border_bottom = if active {
        key.accent.alpha(0.22)
    } else if is_empty {
        oklcha(1.0, 0.0, 0.0, 0.02)
    } else {
        oklcha(1.0, 0.0, 0.0, 0.03)
    };

    let icon_color = if active {
        key.accent
    } else if is_nav {
        key.accent.alpha(0.85)
    } else if is_empty {
        oklcha(1.0, 0.0, 0.0, 0.10)
    } else {
        oklch(0.90, 0.006, 260.0)
    };

    let label_color = if active {
        oklch(0.98, 0.003, 260.0)
    } else if is_empty {
        oklcha(1.0, 0.0, 0.0, 0.0)
    } else {
        oklch(0.80, 0.008, 260.0)
    };

    let top_row = row()
        .w_full()
        .center()
        .child(if active {
            el().key(format!("deck-dot-{idx}"))
                .size(5.0, 5.0)
                .radius(2.5)
                .fill(key.accent)
        } else if is_nav {
            el().key(format!("deck-dot-{idx}"))
                .size(5.0, 5.0)
                .radius(2.5)
                .fill(key.accent.alpha(0.45))
        } else {
            el().key(format!("deck-dot-{idx}")).size(5.0, 5.0)
        })
        .child(spacer())
        .child(
            text(if is_empty { "" } else { &slot_num })
                .font(tok::TextStyle::new(10.5, 500.0).mono())
                .color(if active {
                    key.accent.alpha(0.75)
                } else {
                    oklcha(1.0, 0.0, 0.0, 0.22)
                }),
        );

    let glyph_color = if is_loading {
        icon_color.alpha(0.55)
    } else {
        icon_color
    };
    let glyph = icon_el(&key.kind, 26.0, glyph_color);
    let label_str = if is_loading {
        "Loading…"
    } else if is_empty {
        ""
    } else {
        &key.label
    };
    let label = text(label_str)
        .font(tok::TextStyle::new(12.5, 510.0).tracking(-0.1))
        .max_w(82.0)
        .truncate()
        .color(if is_loading {
            key.accent.alpha(0.95)
        } else {
            label_color
        });

    let mut tile = col()
        .key(format!("deck-key-{idx}"))
        .role("button")
        .label(&key.label)
        .focusable(!is_empty)
        .size(104.0, 104.0)
        .radius(11.0)
        .gradient(top_fill, bottom_fill)
        .border_gradient(1.0, border_top, border_bottom)
        .scale(if is_pressed { 0.94 } else { 1.0 })
        .press_scale(if is_empty { 1.0 } else { 0.96 })
        .motion(motion::SNAPPY)
        .padding(11.0)
        .center()
        .on_press(move |s: &mut DeckState| s.press_key(idx))
        .child(top_row)
        .child(spacer())
        .child(glyph)
        .child(spacer())
        .child(label);

    // Raycast-style subtle spotlight glow behind active tiles.
    if active {
        tile = tile.radial_gradient(
            [0.5, 0.42],
            58.0,
            [(0.0, key.accent.alpha(0.20)), (1.0, key.accent.alpha(0.0))],
        );
    }

    tile
}

pub fn view(s: &DeckState, _: &mut Cx<DeckState>) -> El {
    let frame = s.current_frame();
    let deck = s.current_deck();
    let layer = s.current_layer();

    let title_text = if s.in_subdeck() {
        format!("← {}", deck.name)
    } else {
        deck.name.clone()
    };

    // Minimal layer indicator dots (Raycast/Linear pagination style).
    let mut layer_dots = row().key("layer-dots").center().gap(4.0);
    if deck.layers.len() > 1 {
        for i in 0..deck.layers.len() {
            let is_cur = i == frame.layer_idx;
            layer_dots = layer_dots.child(
                el().key(format!("layer-dot-{i}"))
                    .size(if is_cur { 12.0 } else { 5.0 }, 5.0)
                    .radius(2.5)
                    .fill(if is_cur {
                        oklcha(1.0, 0.0, 0.0, 0.72)
                    } else {
                        oklcha(1.0, 0.0, 0.0, 0.18)
                    }),
            );
        }
    }

    let header = row()
        .key("deck-header")
        .w_full()
        .px(4.0)
        .center()
        .gap(6.0)
        .child(
            text(title_text)
                .font(tok::TextStyle::new(12.0, 510.0).tracking(-0.15))
                .color(oklcha(1.0, 0.0, 0.0, 0.88)),
        )
        .child(
            text(
                s.error_message
                    .as_deref()
                    .unwrap_or(layer.name.as_str()),
            )
            .font(tok::TextStyle::new(11.0, 450.0))
            .grow(1.0)
            .min_w(0.0)
            .truncate()
            .color(if s.error_message.is_some() {
                oklch(0.72, 0.16, 25.0)
            } else {
                oklcha(1.0, 0.0, 0.0, 0.36)
            }),
        )
        .child(layer_dots);

    let mut grid = col().key("deck-grid").gap(10.0);
    for r in 0..3 {
        let mut grid_row = row().key(format!("deck-row-{r}")).gap(10.0);
        for c in 0..3 {
            let idx = r * 3 + c;
            let is_pressed = s.pressed_key == Some(idx);
            let is_loading = s.loading_key == Some(idx);
            grid_row = grid_row.child(deck_key_view(idx, &layer.keys[idx], is_pressed, is_loading));
        }
        grid = grid.child(grid_row);
    }

    let deck_shell = col()
        .key("deck-shell")
        .w(360.0)
        .padding(14.0)
        .gap(12.0)
        .radius(14.0)
        .offset(0.0, if s.visible { 0.0 } else { 440.0 })
        .offset_motion(Spring::new(0.30, 0.10))
        .opacity(if s.visible { 1.0 } else { 0.0 })
        .opacity_motion(Spring::new(0.18, 0.0))
        .gradient(
            oklch(0.145, 0.008, 260.0),
            oklch(0.125, 0.007, 260.0),
        )
        .border_gradient(
            1.0,
            oklcha(1.0, 0.0, 0.0, 0.13),
            oklcha(1.0, 0.0, 0.0, 0.04),
        )
        .shadow(8.0, 20.0, oklcha(0.0, 0.0, 0.0, 0.55))
        .child(header)
        .child(grid);

    col()
        .key("root")
        .w_full()
        .h_full()
        .center()
        .child(deck_shell)
}
