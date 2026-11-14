use eframe::egui::*;

pub const BASE: Color32 = Color32::from_rgb(17, 18, 21);
pub const PANEL: Color32 = Color32::from_rgb(23, 24, 28);
pub const BORDER: Color32 = Color32::from_rgb(44, 45, 51);
pub const SOFT: Color32 = Color32::from_rgb(151, 151, 162);
pub const CREAM: Color32 = Color32::from_rgb(245, 242, 234);
pub const AMBER: Color32 = Color32::from_rgb(245, 185, 90);
pub const INK: Color32 = Color32::from_rgb(43, 29, 12);

pub fn install(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Geist".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!("../assets/Geist.ttf"))),
    );
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "Geist".into());
    ctx.set_fonts(fonts);

    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(CREAM);
    visuals.panel_fill = BASE;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(13, 14, 16);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.bg_fill = PANEL;
    visuals.widgets.inactive.weak_bg_fill = PANEL;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(39, 39, 44);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(57, 48, 37);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, AMBER.gamma_multiply(0.55));
    visuals.widgets.active.bg_fill = AMBER;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, INK);
    visuals.selection.bg_fill = Color32::from_rgb(90, 61, 25);
    visuals.slider_trailing_fill = true;
    ctx.set_visuals(visuals);
}

#[derive(Clone, Copy)]
pub enum Icon {
    Link,
    Queue,
    Library,
    Play,
    Pause,
    Back,
    Forward,
    Stop,
    Volume(bool),
    Fullscreen,
    Captions,
    Audio,
}

pub fn icon_button(ui: &mut Ui, icon: Icon, size: f32, title: &str, accent: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let response = response.on_hover_text(title);
    let active = ui.is_enabled();
    let fill = if accent && active {
        if response.hovered() {
            AMBER.linear_multiply(1.12)
        } else {
            AMBER
        }
    } else if response.hovered() && active {
        Color32::from_rgb(44, 44, 51)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, if accent { size / 2.0 } else { 9.0 }, fill);
    let color = if accent && active {
        INK
    } else if active {
        CREAM
    } else {
        SOFT.gamma_multiply(0.38)
    };
    draw_icon(ui.painter(), rect.center(), icon, color);
    response
}

fn draw_icon(p: &Painter, center: Pos2, icon: Icon, color: Color32) {
    let pos = |x, y| center + vec2(x, y);
    let stroke = Stroke::new(1.65_f32, color);
    let line =
        |a: (f32, f32), b: (f32, f32)| p.line_segment([pos(a.0, a.1), pos(b.0, b.1)], stroke);
    let path = |points: &[(f32, f32)]| {
        p.add(Shape::line(
            points.iter().map(|&(x, y)| pos(x, y)).collect(),
            stroke,
        ));
    };
    let polygon = |points: &[(f32, f32)]| {
        p.add(Shape::convex_polygon(
            points.iter().map(|&(x, y)| pos(x, y)).collect(),
            color,
            Stroke::NONE,
        ));
    };
    let box_shape = |min: (f32, f32), max: (f32, f32), round| {
        p.rect_stroke(
            Rect::from_min_max(pos(min.0, min.1), pos(max.0, max.1)),
            round,
            stroke,
            StrokeKind::Inside,
        );
    };
    match icon {
        Icon::Play => polygon(&[(-4.5, -7.5), (7.0, 0.0), (-4.5, 7.5)]),
        Icon::Pause => {
            for x in [-4.5, 1.5] {
                p.rect_filled(
                    Rect::from_min_size(pos(x, -6.0), vec2(3.0, 12.0)),
                    0.8,
                    color,
                );
            }
        }
        Icon::Stop => {
            p.rect_filled(
                Rect::from_center_size(center, Vec2::splat(10.0)),
                1.5,
                color,
            );
        }
        Icon::Back | Icon::Forward => {
            let sign = if matches!(icon, Icon::Back) {
                -1.0
            } else {
                1.0
            };
            polygon(&[(-sign * 4.0, -5.5), (sign * 4.0, 0.0), (-sign * 4.0, 5.5)]);
            line((sign * 6.0, -5.5), (sign * 6.0, 5.5));
        }
        Icon::Library => {
            for y in [-7.0, 1.0] {
                for x in [-7.0, 1.0] {
                    box_shape((x, y), (x + 6.0, y + 6.0), 1.1);
                }
            }
        }
        Icon::Link => {
            path(&[
                (-2.0, 4.0),
                (-4.0, 6.0),
                (-7.0, 5.0),
                (-7.0, 2.0),
                (-3.0, -2.0),
                (0.0, -2.0),
            ]);
            path(&[
                (2.0, -4.0),
                (4.0, -6.0),
                (7.0, -5.0),
                (7.0, -2.0),
                (3.0, 2.0),
                (0.0, 2.0),
            ]);
            line((-2.0, 2.0), (2.0, -2.0));
        }
        Icon::Queue => {
            for y in [-5.0, 0.0, 5.0] {
                line((-7.0, y), (-6.0, y));
                line((-3.0, y), (7.0, y));
            }
        }
        Icon::Volume(muted) => {
            path(&[
                (-7.5, -2.5),
                (-4.0, -2.5),
                (0.0, -6.0),
                (0.0, 6.0),
                (-4.0, 2.5),
                (-7.5, 2.5),
                (-7.5, -2.5),
            ]);
            if muted {
                line((3.5, -2.5), (8.0, 2.5));
                line((3.5, 2.5), (8.0, -2.5));
            } else {
                path(&[(3.0, -3.0), (4.5, -1.3), (4.5, 1.3), (3.0, 3.0)]);
                path(&[(5.5, -5.0), (7.5, -2.4), (7.5, 2.4), (5.5, 5.0)]);
            }
        }
        Icon::Fullscreen => {
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                path(&[(3.0 * x, 7.0 * y), (7.0 * x, 7.0 * y), (7.0 * x, 3.0 * y)]);
            }
        }
        Icon::Captions => {
            box_shape((-9.0, -6.5), (9.0, 6.5), 2.0);
            for x in [-5.0, 2.0] {
                path(&[(x + 3.0, -2.0), (x, -2.0), (x, 2.0), (x + 3.0, 2.0)]);
            }
        }
        Icon::Audio => {
            path(&[
                (-7.0, 1.0),
                (-7.0, -2.0),
                (-4.0, -6.0),
                (4.0, -6.0),
                (7.0, -2.0),
                (7.0, 1.0),
            ]);
            box_shape((-8.0, 0.0), (-4.0, 6.0), 1.5);
            box_shape((4.0, 0.0), (8.0, 6.0), 1.5);
        }
    }
}

pub fn text(text: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(text.into()).size(size).color(color)
}

pub fn button(label: &str, accent: bool) -> Button<'_> {
    Button::new(text(label, 14.0, if accent { INK } else { CREAM }))
        .fill(if accent { AMBER } else { PANEL })
        .stroke(Stroke::new(1.0_f32, if accent { AMBER } else { BORDER }))
        .corner_radius(9)
}
