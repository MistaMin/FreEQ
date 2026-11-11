//! Graph editor: FFT behind the EQ curve, a node for every band, and the
//! gain / frequency / Q strip underneath.

use openplugin::dri::goodlookindri::color::Color;
use openplugin::dri::goodlookindri::scene::{Point, Rect as GlRect, Scene};
use openplugin::dri::goodlookindri::spectrum_draw::{SpectrumSettings, SpectrumStyle, SpectrumView};
use openplugin::dri::DrawList;
use openplugin::prelude::Plugin;

use crate::params::{
    freq_index, gain_index, q_index, type_index, BANDS, BAND_COLOR, PARAMS, PHASE_INDEX, TRIM_INDEX,
    TYPE_CHOICES,
};
use crate::plugin::EqPlugin;

const DESIGN_W: f32 = 980.0;
const DESIGN_H: f32 = 640.0;

/// Remembers which control a pointer gesture is moving.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gesture {
    node: Option<usize>,
    /// Band index and row: 0 gain, 1 frequency, 2 Q. `band == BANDS` is trim.
    value: Option<(usize, usize)>,
    start: f32,
    origin_y: f32,
}

/// Press. Returns whether a control was hit.
pub fn pointer_down(plugin: &mut EqPlugin, width: f32, height: f32, x: f32, y: f32, gesture: &mut Gesture) -> bool {
    *gesture = Gesture::default();
    let (x, y) = to_design(width, height, x, y);
    let scale = scale_of(DESIGN_W, DESIGN_H);
    if let Some(band) = node_at(plugin, scale, x, y) {
        gesture.node = Some(band);
        return true;
    }
    if let Some((band, row)) = value_at(scale, x, y) {
        let index = if band == BANDS {
            TRIM_INDEX
        } else {
            [gain_index(band), freq_index(band), q_index(band)][row]
        };
        gesture.value = Some((band, row));
        gesture.start = value(plugin, index);
        gesture.origin_y = y;
        return true;
    }
    if let Some(band) = type_at(scale, x, y) {
        let index = type_index(band);
        let next = (value(plugin, index).round() as i32 + 1).rem_euclid(5) as f32;
        let _ = plugin.set_param(PARAMS[index].id, next);
        return true;
    }
    if let Some(mode) = phase_at(x, y) {
        let _ = plugin.set_param(PARAMS[PHASE_INDEX].id, mode as f32);
        return true;
    }
    false
}

/// Drag. Frequency and gain follow a graph node. Value boxes and trim follow vertical movement.
pub fn pointer_drag(plugin: &mut EqPlugin, width: f32, height: f32, x: f32, y: f32, gesture: &Gesture) {
    let (x, y) = to_design(width, height, x, y);
    let scale = scale_of(DESIGN_W, DESIGN_H);
    if let Some(band) = gesture.node {
        let _ = plugin.set_param(PARAMS[freq_index(band)].id, x_to_freq(scale.graph, x));
        let kind = value(plugin, type_index(band)).round() as i32;
        if kind != 1 && kind != 2 {
            let _ = plugin.set_param(PARAMS[gain_index(band)].id, y_to_db(scale.graph, y));
        }
    } else if let Some((band, row)) = gesture.value {
        let delta = gesture.origin_y - y;
        if band == BANDS {
            let next = (gesture.start + delta * 0.05).clamp(-24.0, 24.0);
            let _ = plugin.set_param(PARAMS[TRIM_INDEX].id, next);
            return;
        }
        let index = [gain_index(band), freq_index(band), q_index(band)][row];
        let next = match row {
            0 => (gesture.start + delta * 0.05).clamp(-18.0, 18.0),
            1 => (gesture.start + delta * 8.0).clamp(20.0, 20000.0),
            _ => (gesture.start + delta * 0.02).clamp(0.2, 8.0),
        };
        let _ = plugin.set_param(PARAMS[index].id, next);
    }
}

/// GoodLookin spectrum. The FFT is `goodlookindri::spectrum`.
/// Multicolour is the full-colour column fill. Peak-hold is off because that stroke walks
/// every pixel of the plot against every column, which stalls the shell's software rasteriser.
pub fn spectrum_view() -> SpectrumView {
    let mut settings = SpectrumSettings::default();
    settings.style = SpectrumStyle::Multicolour;
    settings.peak_hold = false;
    settings.opacity = 0.8;
    settings.tilt = 3.0;
    SpectrumView::new(settings)
}

/// Draws the editor into `scene`. `spectrum` is the DRI analyser; pass `None` to leave the plot empty behind the curve.
/// The design is 980×640. A resize scales it uniformly and centers it, so type and nodes stay round.
pub fn paint(plugin: &EqPlugin, scene: &mut Scene, width: f32, height: f32, spectrum: Option<&mut SpectrumView>) {
    let place = place_of(width, height);
    let paper = rgba(0.91, 0.89, 0.84, 1.0);
    let ink = rgba(0.22, 0.22, 0.24, 1.0);
    let muted = rgba(0.45, 0.45, 0.48, 1.0);
    fill(scene, 0.0, 0.0, width, height, paper);
    text(scene, &place, "FreEQ", 28.0, 30.0, 22.0, rgba(0.35, 0.55, 0.42, 1.0), false);

    let plot = Rect { x: 24.0, y: 52.0, w: 730.0, h: 360.0 };
    draw_plot(scene, &place, plugin, plot, spectrum);
    draw_meters(scene, &place, plugin, ink, muted);
    draw_phase(scene, &place, plugin, ink);
    draw_bands(scene, &place, plugin, ink);
}

#[derive(Clone, Copy)]
struct Place {
    s: f32,
    ox: f32,
    oy: f32,
}

fn place_of(width: f32, height: f32) -> Place {
    let s = (width / DESIGN_W).min(height / DESIGN_H).max(0.001);
    Place {
        s,
        ox: (width - DESIGN_W * s) * 0.5,
        oy: (height - DESIGN_H * s) * 0.5,
    }
}

fn to_design(width: f32, height: f32, x: f32, y: f32) -> (f32, f32) {
    let place = place_of(width, height);
    ((x - place.ox) / place.s, (y - place.oy) / place.s)
}

pub fn render(plugin: &EqPlugin, width: f32, height: f32, spectrum: Option<&mut SpectrumView>) -> DrawList {
    let mut list = DrawList::new();
    paint(plugin, &mut list.scene, width, height, spectrum);
    list
}

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::rgba(
        (r.clamp(0.0, 1.0) * 255.0).round() as u32,
        (g.clamp(0.0, 1.0) * 255.0).round() as u32,
        (b.clamp(0.0, 1.0) * 255.0).round() as u32,
        (a.clamp(0.0, 1.0) * 255.0).round() as u32,
    )
}

fn dx(place: &Place, x: f32) -> f32 {
    place.ox + x * place.s
}

fn dy(place: &Place, y: f32) -> f32 {
    place.oy + y * place.s
}

fn ds(place: &Place, v: f32) -> f32 {
    v * place.s
}

fn fill(scene: &mut Scene, x: f32, y: f32, w: f32, h: f32, color: Color) {
    scene.fill_rect(GlRect::new(x, y, w, h), color, color, true);
}

fn round(scene: &mut Scene, place: &Place, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Color) {
    scene.fill_round(
        GlRect::new(dx(place, x), dy(place, y), ds(place, w), ds(place, h)),
        ds(place, radius),
        color,
        color,
        true,
    );
}

fn circle(scene: &mut Scene, place: &Place, cx: f32, cy: f32, radius: f32, color: Color) {
    let r = ds(place, radius);
    scene.fill_ellipse_solid(GlRect::new(dx(place, cx) - r, dy(place, cy) - r, r * 2.0, r * 2.0), color);
}

fn rule(scene: &mut Scene, place: &Place, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
    scene.line(Point::new(dx(place, x1), dy(place, y1)), Point::new(dx(place, x2), dy(place, y2)), color, ds(place, width));
}

fn text(scene: &mut Scene, place: &Place, label: impl Into<String>, x: f32, y: f32, size: f32, color: Color, center: bool) {
    scene.text(Point::new(dx(place, x), dy(place, y)), label, color, ds(place, size), center);
}

/// Right edge of the text sits on `right`. The advance estimate stays inside the gutter for "-12".
fn text_right(scene: &mut Scene, place: &Place, label: &str, right: f32, y: f32, size: f32, color: Color) {
    let advance = size * 0.58 * label.chars().count() as f32;
    text(scene, place, label, right - advance, y, size, color, false);
}

fn shown(value: f32) -> f32 {
    if value.abs() < 5.0e-4 { 0.0 } else { value }
}

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn draw_plot(scene: &mut Scene, place: &Place, plugin: &EqPlugin, plot: Rect, mut spectrum: Option<&mut SpectrumView>) {
    round(scene, place, plot.x, plot.y, plot.w, plot.h, 16.0, rgba(0.78, 0.78, 0.76, 1.0));
    let inner = Rect { x: plot.x + 14.0, y: plot.y + 14.0, w: plot.w - 28.0, h: plot.h - 28.0 };
    round(scene, place, inner.x, inner.y, inner.w, inner.h, 8.0, rgba(0.08, 0.14, 0.22, 1.0));
    let graph = Rect { x: inner.x + 28.0, y: inner.y + 16.0, w: inner.w - 56.0, h: inner.h - 36.0 };
    if let Some(view) = spectrum.as_mut() {
        view.paint(
            scene,
            GlRect::new(dx(place, graph.x), dy(place, graph.y), ds(place, graph.w), ds(place, graph.h)),
            20.0,
            20000.0,
        );
    }
    text_right(scene, place, "View", graph.x + graph.w - 8.0, graph.y + 12.0, 11.0, rgba(0.75, 0.78, 0.82, 1.0));

    for db in [-12.0, -6.0, 0.0, 6.0, 12.0] {
        let y = gain_y(graph, db);
        rule(scene, place, graph.x, y, graph.x + graph.w, y, rgba(1.0, 1.0, 1.0, 0.08), 1.0);
        let label = if db > 0.0 { format!("+{db:.0}") } else { format!("{db:.0}") };
        text_right(scene, place, &label, graph.x - 4.0, y, 10.0, rgba(0.75, 0.78, 0.82, 1.0));
    }
    for freq in [31.0, 125.0, 500.0, 2000.0, 8000.0] {
        let x = freq_x(graph, freq);
        rule(scene, place, x, graph.y, x, graph.y + graph.h, rgba(1.0, 1.0, 1.0, 0.06), 1.0);
        let label = if freq >= 1000.0 { format!("{}K", (freq / 1000.0) as i32) } else { format!("{freq:.0}") };
        text(scene, place, label, x, graph.y + graph.h + 12.0, 11.0, rgba(0.75, 0.78, 0.82, 1.0), true);
    }

    draw_curve(scene, place, plugin, graph);

    for band in 0..BANDS {
        let freq = value(plugin, freq_index(band));
        let kind = value(plugin, type_index(band)).round() as i32;
        let gain = if kind == 1 || kind == 2 { plugin.magnitude_db(freq) } else { value(plugin, gain_index(band)) };
        let x = freq_x(graph, freq);
        let y = gain_y(graph, gain.clamp(-18.0, 18.0));
        let (r, g, b) = BAND_COLOR[band];
        circle(scene, place, x, y, 11.0, rgba(r, g, b, 1.0));
        text(scene, place, format!("{}", band + 1), x, y, 12.0, rgba(1.0, 1.0, 1.0, 1.0), true);
    }
}

fn draw_curve(scene: &mut Scene, place: &Place, plugin: &EqPlugin, graph: Rect) {
    let steps = 96;
    let mut previous: Option<(f32, f32)> = None;
    let gold = rgba(0.93, 0.78, 0.28, 1.0);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let freq = 20.0 * (20000.0_f32 / 20.0).powf(t);
        let db = plugin.magnitude_db(freq).clamp(-18.0, 18.0);
        let x = graph.x + t * graph.w;
        let y = gain_y(graph, db);
        if let Some((px, py)) = previous {
            rule(scene, place, px, py, x, y, gold, 2.2);
        }
        previous = Some((x, y));
    }
}

fn draw_meters(scene: &mut Scene, place: &Place, plugin: &EqPlugin, ink: Color, muted: Color) {
    let meters = plugin.output_db();
    let tracks = [824.0, 888.0];
    for (index, db) in meters.iter().enumerate() {
        text(scene, place, format!("{:.1}", shown(db.max(-80.0))), tracks[index] + 4.0, 78.0, 12.0, muted, true);
        let top = 100.0;
        let h = 168.0;
        round(scene, place, tracks[index], top, 8.0, h, 3.0, rgba(0.55, 0.62, 0.78, 1.0));
        let t = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let cap_y = (top + h * (1.0 - t)).clamp(top, top + h) - 8.0;
        round(scene, place, tracks[index] - 6.0, cap_y, 20.0, 16.0, 8.0, rgba(0.95, 0.62, 0.18, 1.0));
    }
    text(scene, place, "Output", 860.0, 286.0, 13.0, ink, true);
    text(scene, place, "Trim", 860.0, 366.0, 13.0, ink, true);
    round(scene, place, 820.0, 378.0, 80.0, 24.0, 8.0, rgba(0.82, 0.82, 0.8, 1.0));
    text(scene, place, format!("{:.1}", shown(value(plugin, TRIM_INDEX))), 860.0, 390.0, 13.0, ink, true);
}

const PHASE_LABELS: [&str; 3] = ["MP", "LP", "ZL"];

fn phase_button(mode: usize) -> Rect {
    Rect { x: 786.0 + mode as f32 * 64.0, y: 430.0, w: 56.0, h: 28.0 }
}

fn draw_phase(scene: &mut Scene, place: &Place, plugin: &EqPlugin, ink: Color) {
    let selected = value(plugin, PHASE_INDEX).round().clamp(0.0, 2.0) as usize;
    text(scene, place, "Phase", 860.0, 416.0, 13.0, ink, true);
    for mode in 0..3 {
        let button = phase_button(mode);
        let on = mode == selected;
        let fill = if on { rgba(0.28, 0.38, 0.34, 1.0) } else { rgba(0.95, 0.95, 0.94, 1.0) };
        let label = if on { rgba(1.0, 1.0, 1.0, 1.0) } else { ink };
        round(scene, place, button.x, button.y, button.w, button.h, 8.0, fill);
        text(scene, place, PHASE_LABELS[mode], button.x + button.w * 0.5, button.y + button.h * 0.5, 13.0, label, true);
    }
}

fn phase_at(x: f32, y: f32) -> Option<usize> {
    (0..3).find(|&mode| contains(phase_button(mode), x, y))
}

fn draw_bands(scene: &mut Scene, place: &Place, plugin: &EqPlugin, ink: Color) {
    let y = 430.0;
    for band in 0..BANDS {
        let x = 24.0 + band as f32 * 122.0;
        let w = 112.0;
        let (r, g, b) = BAND_COLOR[band];
        round(scene, place, x, y, w, 190.0, 12.0, rgba(0.86, 0.86, 0.84, 1.0));
        round(scene, place, x + 8.0, y + 8.0, w - 16.0, 22.0, 8.0, rgba(r, g, b, 1.0));
        text(scene, place, format!("{}", band + 1), x + w * 0.5, y + 19.0, 13.0, rgba(1.0, 1.0, 1.0, 1.0), true);
        let rows = [("G", gain_index(band), 1), ("F", freq_index(band), 0), ("Q", q_index(band), 2)];
        for (row, (name, index, digits)) in rows.iter().enumerate() {
            let ry = y + 40.0 + row as f32 * 28.0;
            text(scene, place, *name, x + 14.0, ry + 8.0, 12.0, ink, true);
            round(scene, place, x + 26.0, ry - 2.0, w - 36.0, 20.0, 6.0, rgba(0.8, 0.8, 0.78, 1.0));
            let number = shown(value(plugin, *index));
            let label = match *digits {
                0 => format!("{:.0}", number.clamp(20.0, 20000.0)),
                1 => format!("{:.1}", number),
                _ => format!("{:.2}", number),
            };
            text(scene, place, label, x + 26.0 + (w - 36.0) * 0.5, ry + 8.0, 12.0, ink, true);
        }
        let kind = value(plugin, type_index(band)).round().clamp(0.0, 4.0) as usize;
        round(scene, place, x + 8.0, y + 128.0, w - 16.0, 28.0, 8.0, rgba(0.95, 0.95, 0.94, 1.0));
        text(scene, place, TYPE_CHOICES[kind], x + w * 0.5, y + 142.0, 11.0, ink, true);
    }
}

fn value(plugin: &EqPlugin, index: usize) -> f32 {
    plugin.param(PARAMS[index].id).unwrap_or(PARAMS[index].default)
}

#[derive(Clone, Copy)]
struct Scale {
    sx: f32,
    sy: f32,
    graph: Rect,
}

fn scale_of(width: f32, height: f32) -> Scale {
    let sx = width / DESIGN_W;
    let sy = height / DESIGN_H;
    let plot = Rect { x: 24.0 * sx, y: 52.0 * sy, w: 730.0 * sx, h: 360.0 * sy };
    let inner = Rect { x: plot.x + 14.0, y: plot.y + 14.0, w: plot.w - 28.0, h: plot.h - 28.0 };
    let graph = Rect { x: inner.x + 28.0, y: inner.y + 16.0, w: inner.w - 56.0, h: inner.h - 36.0 };
    Scale { sx, sy, graph }
}

fn node_at(plugin: &EqPlugin, scale: Scale, x: f32, y: f32) -> Option<usize> {
    for band in (0..BANDS).rev() {
        let freq = value(plugin, freq_index(band));
        let kind = value(plugin, type_index(band)).round() as i32;
        let db = if kind == 1 || kind == 2 {
            plugin.magnitude_db(freq)
        } else {
            value(plugin, gain_index(band))
        };
        let nx = freq_x(scale.graph, freq);
        let ny = gain_y(scale.graph, db.clamp(-18.0, 18.0));
        let dx = x - nx;
        let dy = y - ny;
        if dx * dx + dy * dy < 14.0 * 14.0 {
            return Some(band);
        }
    }
    None
}

fn value_at(scale: Scale, x: f32, y: f32) -> Option<(usize, usize)> {
    let trim = Rect { x: 820.0 * scale.sx, y: 378.0 * scale.sy, w: 80.0, h: 24.0 };
    if contains(trim, x, y) {
        return Some((BANDS, 0));
    }
    for band in 0..BANDS {
        let left = (24.0 + band as f32 * 122.0) * scale.sx;
        let top = 430.0 * scale.sy;
        let w = 112.0 * scale.sx;
        for row in 0..3 {
            let box_y = top + (40.0 + row as f32 * 28.0) * scale.sy - 2.0;
            if contains(Rect { x: left + 26.0 * scale.sx, y: box_y, w: w - 36.0 * scale.sx, h: 20.0 }, x, y) {
                return Some((band, row));
            }
        }
    }
    None
}

fn type_at(scale: Scale, x: f32, y: f32) -> Option<usize> {
    for band in 0..BANDS {
        let left = (24.0 + band as f32 * 122.0) * scale.sx;
        let top = 430.0 * scale.sy + 128.0 * scale.sy;
        let w = 112.0 * scale.sx;
        if contains(Rect { x: left + 8.0 * scale.sx, y: top, w: w - 16.0 * scale.sx, h: 28.0 * scale.sy }, x, y) {
            return Some(band);
        }
    }
    None
}

fn contains(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.x && y >= rect.y && x <= rect.x + rect.w && y <= rect.y + rect.h
}

fn x_to_freq(graph: Rect, x: f32) -> f32 {
    let t = ((x - graph.x) / graph.w).clamp(0.0, 1.0);
    20.0 * (20000.0_f32 / 20.0).powf(t)
}

fn y_to_db(graph: Rect, y: f32) -> f32 {
    let t = ((y - graph.y) / graph.h).clamp(0.0, 1.0);
    18.0 - t * 36.0
}

fn freq_x(graph: Rect, freq: f32) -> f32 {
    let t = (freq.max(20.0) / 20.0).ln() / (20000.0_f32 / 20.0).ln();
    graph.x + t.clamp(0.0, 1.0) * graph.w
}

fn gain_y(graph: Rect, db: f32) -> f32 {
    let t = (18.0 - db.clamp(-18.0, 18.0)) / 36.0;
    graph.y + t * graph.h
}
