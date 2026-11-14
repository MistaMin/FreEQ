//! Graph editor: FFT behind the EQ curve, a node for every band, a card per band with its
//! filter shape drawn as a symbol, and input and output meters beside the plot.
//!
//! A click on empty plot steps the FFT from live to frozen to off and back to live.
//!
//! Everything is laid out in a 980×640 design space. `paint` scales that space uniformly into
//! the window, and the pointer functions map window coordinates back into it, so one set of
//! rectangles serves both drawing and hit-testing.

use goodlookindri_shell::text as type_metrics;
use openplugin::dri::goodlookindri::color::Color;
use openplugin::dri::goodlookindri::scene::{Point, Rect as GlRect, Scene};
use openplugin::dri::goodlookindri::spectrum::{Analyser, SampleRing};
use openplugin::dri::goodlookindri::spectrum_draw::{SpectrumSettings, SpectrumStyle, SpectrumView};
use openplugin::dri::DrawList;
use openplugin::prelude::Plugin;

use crate::dsp::MeterReading;
use crate::params::{
    freq_index, gain_index, q_index, type_index, BANDS, BAND_COLOR, PARAMS, PHASE_INDEX, TRIM_INDEX,
};
use crate::plugin::EqPlugin;

const DESIGN_W: f32 = 980.0;
const DESIGN_H: f32 = 640.0;

// Palette, `0xRRGGBB`. Graphite panels on a near-black ground, one mint accent, and a band hue
// per filter from `BAND_COLOR`.
const GROUND_TOP: u32 = 0x171A23;
const GROUND_BOTTOM: u32 = 0x0D0F15;
const PANEL: u32 = 0x1B1F2A;
const PANEL_EDGE: u32 = 0x2B3142;
const WELL: u32 = 0x0F1218;
const WELL_EDGE: u32 = 0x272C3A;
const PLOT_GROUND: u32 = 0x0C0E14;
const INK: u32 = 0xE9ECF3;
const MUTED: u32 = 0x8891A7;
const FAINT: u32 = 0x565E73;
const ACCENT: u32 = 0x5EEAD4;
const ACCENT_DEEP: u32 = 0x38BDF8;
const CURVE: u32 = 0xF5F8FC;
const DARK_INK: u32 = 0x0D0F15;
const CLIP_RED: u32 = 0xFF5C6C;

/// Analyser colours from 20 Hz to 20 kHz: indigo through blue to mint, so the band hues stay
/// the brightest thing on the plot.
const SPECTRUM_RANGE: [u32; 7] = [
    0xff5b5bd6, 0xff4f74e8, 0xff438ef0, 0xff3aa8ea, 0xff3cc2d8, 0xff4fd8c0, 0xff6ee7b0,
];

// Layout, in design units.
const PLOT: Rect = Rect { x: 24.0, y: 52.0, w: 730.0, h: 360.0 };
const GRAPH: Rect = Rect { x: 66.0, y: 82.0, w: 646.0, h: 296.0 };
const METERS: Rect = Rect { x: 772.0, y: 52.0, w: 184.0, h: 360.0 };
const OUTPUT: Rect = Rect { x: 772.0, y: 430.0, w: 184.0, h: 190.0 };
const TRIM_BOX: Rect = Rect { x: 788.0, y: 464.0, w: 152.0, h: 34.0 };
const CARD_Y: f32 = 430.0;
const CARD_W: f32 = 115.0;
const CARD_H: f32 = 190.0;
const CARD_PITCH: f32 = 123.0;

const METER_TOP_DB: f32 = 6.0;
const METER_BOTTOM_DB: f32 = -60.0;
const METER_TRACK_Y: f32 = 110.0;
const METER_TRACK_H: f32 = 280.0;
const METER_BAR_W: f32 = 12.0;

fn card(band: usize) -> Rect {
    Rect { x: 24.0 + band as f32 * CARD_PITCH, y: CARD_Y, w: CARD_W, h: CARD_H }
}

/// The button that shows the band's filter shape. A click steps to the next shape.
fn type_button(band: usize) -> Rect {
    let card = card(band);
    Rect { x: card.x + 40.0, y: card.y + 10.0, w: card.w - 50.0, h: 34.0 }
}

/// Row 0 is gain, 1 is frequency, 2 is Q.
fn value_box(band: usize, row: usize) -> Rect {
    let card = card(band);
    Rect { x: card.x + 46.0, y: card.y + 58.0 + row as f32 * 40.0, w: card.w - 56.0, h: 28.0 }
}

fn phase_button(mode: usize) -> Rect {
    Rect { x: 780.0 + mode as f32 * 58.0, y: 536.0, w: 52.0, h: 32.0 }
}

/// Remembers which control a pointer gesture is moving.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gesture {
    node: Option<usize>,
    /// Band index and row: 0 gain, 1 frequency, 2 Q. `band == BANDS` is trim.
    value: Option<(usize, usize)>,
    start: f32,
    origin_y: f32,
    plot: bool,
}

impl Gesture {
    /// The press landed on empty plot, not on a node. Whoever owns the [`Spectrum`] answers
    /// with [`Spectrum::next_mode`].
    pub fn on_plot(&self) -> bool {
        self.plot
    }
}

/// What the FFT behind the curve is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpectrumMode {
    /// Follows the audio.
    #[default]
    Live,
    /// Keeps the picture it had when it was frozen.
    Frozen,
    /// Not drawn and not analysed.
    Off,
}

impl SpectrumMode {
    /// Live, then frozen, then off, then live again.
    pub fn next(self) -> Self {
        match self {
            Self::Live => Self::Frozen,
            Self::Frozen => Self::Off,
            Self::Off => Self::Live,
        }
    }
}

/// The DRI analyser plus the mode a click on the plot has put it in.
pub struct Spectrum {
    view: SpectrumView,
    mode: SpectrumMode,
}

impl Spectrum {
    pub fn mode(&self) -> SpectrumMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: SpectrumMode) {
        self.mode = mode;
    }

    /// Steps to the next mode and returns it.
    pub fn next_mode(&mut self) -> SpectrumMode {
        self.mode = self.mode.next();
        self.mode
    }

    /// Runs the FFT on what the ring has collected. Does nothing unless the mode is live, so a
    /// frozen picture stays as it is and an FFT that is off costs nothing.
    pub fn update(&mut self, ring: &SampleRing, seconds: f32, sample_rate: f64) {
        if self.mode == SpectrumMode::Live {
            self.view.update(ring, seconds, sample_rate);
        }
    }

    pub fn analyser(&self) -> &Analyser {
        self.view.analyser()
    }
}

/// Press. Returns whether a control was hit.
pub fn pointer_down(plugin: &mut EqPlugin, width: f32, height: f32, x: f32, y: f32, gesture: &mut Gesture) -> bool {
    *gesture = Gesture::default();
    let (x, y) = to_design(width, height, x, y);
    if let Some(band) = node_at(plugin, x, y) {
        gesture.node = Some(band);
        return true;
    }
    if contains(GRAPH, x, y) {
        gesture.plot = true;
        return true;
    }
    if let Some((band, row)) = value_at(x, y) {
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
    if let Some(band) = (0..BANDS).find(|&band| contains(type_button(band), x, y)) {
        let index = type_index(band);
        let next = (value(plugin, index).round() as i32 + 1).rem_euclid(5) as f32;
        let _ = plugin.set_param(PARAMS[index].id, next);
        return true;
    }
    if let Some(mode) = (0..3).find(|&mode| contains(phase_button(mode), x, y)) {
        let _ = plugin.set_param(PARAMS[PHASE_INDEX].id, mode as f32);
        return true;
    }
    false
}

/// Drag. Frequency and gain follow a graph node. Value boxes and trim follow vertical movement.
pub fn pointer_drag(plugin: &mut EqPlugin, width: f32, height: f32, x: f32, y: f32, gesture: &Gesture) {
    let (x, y) = to_design(width, height, x, y);
    if let Some(band) = gesture.node {
        let _ = plugin.set_param(PARAMS[freq_index(band)].id, x_to_freq(x));
        if !is_pass(kind_of(plugin, band)) {
            let _ = plugin.set_param(PARAMS[gain_index(band)].id, y_to_db(y));
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
pub fn spectrum_view() -> Spectrum {
    let mut settings = SpectrumSettings::default();
    settings.style = SpectrumStyle::Multicolour;
    settings.peak_hold = false;
    settings.opacity = 0.62;
    settings.tilt = 3.0;
    settings.range = SPECTRUM_RANGE;
    Spectrum { view: SpectrumView::new(settings), mode: SpectrumMode::Live }
}

/// Draws the editor into `scene`. `spectrum` is the DRI analyser; pass `None` to leave the plot empty behind the curve.
/// The design is 980×640. A resize scales it uniformly and centers it, so type and nodes stay round.
pub fn paint(plugin: &EqPlugin, scene: &mut Scene, width: f32, height: f32, spectrum: Option<&mut Spectrum>) {
    let place = place_of(width, height);
    draw_ground(scene, &place, width, height);
    draw_header(scene, &place);
    draw_plot(scene, &place, plugin, spectrum);
    draw_meters(scene, &place, plugin);
    draw_output(scene, &place, plugin);
    draw_bands(scene, &place, plugin);
}

pub fn render(plugin: &EqPlugin, width: f32, height: f32, spectrum: Option<&mut Spectrum>) -> DrawList {
    let mut list = DrawList::new();
    paint(plugin, &mut list.scene, width, height, spectrum);
    list
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

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn cx(self) -> f32 {
        self.x + self.w * 0.5
    }

    fn cy(self) -> f32 {
        self.y + self.h * 0.5
    }

    fn bottom(self) -> f32 {
        self.y + self.h
    }

    fn right(self) -> f32 {
        self.x + self.w
    }
}

fn contains(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.x && y >= rect.y && x <= rect.right() && y <= rect.bottom()
}

fn rgb(hex: u32) -> Color {
    Color(0xff00_0000 | hex)
}

fn rgba(hex: u32, alpha: f32) -> Color {
    rgb(hex).with_alpha(alpha)
}

fn band_color(band: usize) -> Color {
    let (r, g, b) = BAND_COLOR[band];
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u32;
    Color::rgba(byte(r), byte(g), byte(b), 255)
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

fn device(place: &Place, rect: Rect) -> GlRect {
    GlRect::new(dx(place, rect.x), dy(place, rect.y), ds(place, rect.w), ds(place, rect.h))
}

fn point(place: &Place, x: f32, y: f32) -> Point {
    Point::new(dx(place, x), dy(place, y))
}

/// The window ground, lighter at the top.
///
/// The software rasteriser blends a gradient pixel by pixel, which over the whole window costs
/// more than everything else in the editor put together. The two ends are only about a dozen
/// levels apart, so flat strips look the same. Four fifths of the window is then painted over by
/// the panels, so each strip also skips the span of any panel that covers its full height.
fn draw_ground(scene: &mut Scene, place: &Place, width: f32, height: f32) {
    const STRIPS: usize = 20;
    let (top, bottom) = (rgb(GROUND_TOP), rgb(GROUND_BOTTOM));
    // The part of each panel that is solid from edge to edge: everything between its rounded
    // corners. One unit is left under each side so the ground and the panel overlap.
    let mut covered: Vec<GlRect> = vec![(PLOT, 14.0), (METERS, 14.0), (OUTPUT, 12.0)]
        .into_iter()
        .chain((0..BANDS).map(|band| (card(band), 12.0)))
        .map(|(rect, radius)| {
            let inner = Rect { x: rect.x + 1.0, y: rect.y + radius, w: rect.w - 2.0, h: rect.h - radius * 2.0 };
            device(place, inner)
        })
        .collect();
    covered.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut rows: Vec<f32> = (0..=STRIPS).map(|strip| (height * strip as f32 / STRIPS as f32).round()).collect();
    rows.extend(covered.iter().flat_map(|rect| [rect.y.ceil(), rect.bottom().floor()]));
    rows.retain(|y| (0.0..=height).contains(y));
    rows.sort_by(f32::total_cmp);
    rows.dedup();
    for pair in rows.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        let color = top.mix(bottom, ((from + to) * 0.5 / height * STRIPS as f32).floor() / (STRIPS - 1) as f32);
        let mut x = 0.0;
        let span = |from_x: f32, to_x: f32, scene: &mut Scene| {
            if to_x - from_x >= 1.0 {
                scene.fill_rect(GlRect::new(from_x, from, to_x - from_x, to - from), color, color, true);
            }
        };
        for rect in covered.iter().filter(|rect| rect.y <= from && rect.bottom() >= to) {
            span(x, rect.x.ceil(), scene);
            x = x.max(rect.right().floor());
        }
        span(x, width, scene);
    }
}

fn round(scene: &mut Scene, place: &Place, rect: Rect, radius: f32, top: Color, bottom: Color) {
    scene.fill_round(device(place, rect), ds(place, radius), top, bottom, true);
}

/// A one-unit border. Four lines and four arcs, because a stroked round rectangle makes the
/// software rasteriser visit every pixel inside it.
fn outline(scene: &mut Scene, place: &Place, rect: Rect, radius: f32, color: Color) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let r = radius.min(rect.w * 0.5).min(rect.h * 0.5);
    let width = ds(place, 1.0);
    let (left, top, right, bottom) = (rect.x, rect.y, rect.right(), rect.bottom());
    rule(scene, place, left + r, top, right - r, top, color, 1.0);
    rule(scene, place, left + r, bottom, right - r, bottom, color, 1.0);
    rule(scene, place, left, top + r, left, bottom - r, color, 1.0);
    rule(scene, place, right, top + r, right, bottom - r, color, 1.0);
    let corners = [
        (left + r, top + r, PI),
        (right - r, top + r, PI + FRAC_PI_2),
        (right - r, bottom - r, 0.0),
        (left + r, bottom - r, FRAC_PI_2),
    ];
    for (cx, cy, from) in corners {
        scene.stroke_arc(point(place, cx, cy), ds(place, r), from, from + FRAC_PI_2, color, width);
    }
}

/// A raised card: the surface every group of controls sits on.
fn panel(scene: &mut Scene, place: &Place, rect: Rect, radius: f32) {
    round(scene, place, rect, radius, rgb(PANEL), rgb(PANEL));
    outline(scene, place, rect, radius, rgb(PANEL_EDGE));
}

/// A sunken box: value readouts and meter tracks.
fn well(scene: &mut Scene, place: &Place, rect: Rect, radius: f32) {
    round(scene, place, rect, radius, rgb(WELL), rgb(WELL));
    outline(scene, place, rect, radius, rgb(WELL_EDGE));
}

fn circle(scene: &mut Scene, place: &Place, cx: f32, cy: f32, radius: f32, color: Color) {
    let r = ds(place, radius);
    scene.fill_ellipse_solid(GlRect::new(dx(place, cx) - r, dy(place, cy) - r, r * 2.0, r * 2.0), color);
}

fn ring(scene: &mut Scene, place: &Place, cx: f32, cy: f32, radius: f32, color: Color, width: f32) {
    let r = ds(place, radius);
    scene.stroke_ellipse(GlRect::new(dx(place, cx) - r, dy(place, cy) - r, r * 2.0, r * 2.0), color, ds(place, width));
}

fn rule(scene: &mut Scene, place: &Place, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
    scene.line(point(place, x1, y1), point(place, x2, y2), color, ds(place, width));
}

/// `y` is the middle of the line of text. `center` also centers it on `x`.
fn text(scene: &mut Scene, place: &Place, label: impl Into<String>, x: f32, y: f32, size: f32, color: Color, center: bool) {
    scene.text(point(place, x, y), label, color, ds(place, size), center);
}

/// Right edge of the text sits on `right`.
fn text_right(scene: &mut Scene, place: &Place, label: &str, right: f32, y: f32, size: f32, color: Color) {
    text(scene, place, label, right - type_metrics::width(label, size), y, size, color, false);
}

fn shown(value: f32) -> f32 {
    if value.abs() < 5.0e-4 { 0.0 } else { value }
}

fn value(plugin: &EqPlugin, index: usize) -> f32 {
    plugin.param(PARAMS[index].id).unwrap_or(PARAMS[index].default)
}

/// 0 peak, 1 high pass, 2 low pass, 3 low shelf, 4 high shelf.
fn kind_of(plugin: &EqPlugin, band: usize) -> usize {
    value(plugin, type_index(band)).round().clamp(0.0, 4.0) as usize
}

/// High pass and low pass have no gain.
fn is_pass(kind: usize) -> bool {
    kind == 1 || kind == 2
}

fn draw_header(scene: &mut Scene, place: &Place) {
    let mark = Rect { x: 24.0, y: 15.0, w: 26.0, h: 26.0 };
    round(scene, place, mark, 8.0, rgb(ACCENT), rgb(ACCENT_DEEP));
    stroke_shape(scene, place, 0, 1.0, mark.cx(), mark.cy() + 1.0, 16.0, 11.0, rgb(DARK_INK), 2.0);
    let title_x = mark.right() + 10.0;
    text(scene, place, "FreEQ", title_x, 28.0, 21.0, rgb(INK), false);
    let tag_x = title_x + type_metrics::width("FreEQ", 21.0) + 12.0;
    text(scene, place, "SIX-BAND EQUALISER", tag_x, 29.0, 10.5, rgb(FAINT), false);
}

fn draw_plot(scene: &mut Scene, place: &Place, plugin: &EqPlugin, spectrum: Option<&mut Spectrum>) {
    round(scene, place, PLOT, 14.0, rgb(PLOT_GROUND), rgb(PLOT_GROUND));
    outline(scene, place, PLOT, 14.0, rgb(PANEL_EDGE));
    if let Some(spectrum) = spectrum {
        if spectrum.mode != SpectrumMode::Off {
            spectrum.view.paint(scene, device(place, GRAPH), 20.0, 20000.0);
        }
        draw_spectrum_mode(scene, place, spectrum.mode);
    }

    for db in [-12.0, -6.0, 0.0, 6.0, 12.0] {
        let y = gain_y(db);
        let alpha = if db == 0.0 { 0.20 } else { 0.07 };
        rule(scene, place, GRAPH.x, y, GRAPH.right(), y, rgba(INK, alpha), 1.0);
        let label = if db > 0.0 { format!("+{db:.0}") } else { format!("{db:.0}") };
        text_right(scene, place, &label, GRAPH.x - 9.0, y, 10.0, rgb(MUTED));
    }
    for (freq, label) in [
        (50.0, "50"),
        (100.0, "100"),
        (200.0, "200"),
        (500.0, "500"),
        (1000.0, "1k"),
        (2000.0, "2k"),
        (5000.0, "5k"),
        (10000.0, "10k"),
    ] {
        let x = freq_x(freq);
        let decade = freq == 100.0 || freq == 1000.0 || freq == 10000.0;
        rule(scene, place, x, GRAPH.y, x, GRAPH.bottom(), rgba(INK, if decade { 0.10 } else { 0.05 }), 1.0);
        text(scene, place, label, x, GRAPH.bottom() + 14.0, 10.0, rgb(MUTED), true);
    }

    draw_curve(scene, place, plugin);

    for band in 0..BANDS {
        let (x, y) = node_position(plugin, band);
        let color = band_color(band);
        circle(scene, place, x, y, 17.0, color.with_alpha(0.16));
        ring(scene, place, x, y, 13.5, color.with_alpha(0.55), 1.2);
        circle(scene, place, x, y, 10.5, color);
        text(scene, place, format!("{}", band + 1), x, y, 12.0, rgb(DARK_INK), true);
    }
}

/// Says so in the corner of the plot when the FFT is frozen or off. Live shows nothing.
fn draw_spectrum_mode(scene: &mut Scene, place: &Place, mode: SpectrumMode) {
    let (label, color) = match mode {
        SpectrumMode::Live => return,
        SpectrumMode::Frozen => ("FFT FROZEN", rgb(ACCENT)),
        SpectrumMode::Off => ("FFT OFF", rgb(FAINT)),
    };
    let (right, y, size) = (GRAPH.right() - 8.0, GRAPH.y + 11.0, 10.0);
    text_right(scene, place, label, right, y, size, color);
    if mode == SpectrumMode::Frozen {
        // A pause mark in front of the words.
        let left = right - type_metrics::width(label, size) - 11.0;
        for bar in [0.0, 4.5] {
            let mark = Rect { x: left + bar, y: y - 4.0, w: 2.0, h: 8.0 };
            scene.fill_rect(device(place, mark), color, color, true);
        }
    }
}

/// The response line, with a soft fill between it and 0 dB so boosts and cuts read at a glance.
/// The fill is one thin column per scene unit; a single polygon that size would make the
/// software rasteriser test every pixel of the plot against every point of the curve. The
/// columns sit on whole scene units so neighbours meet on a pixel edge and leave no seams.
fn draw_curve(scene: &mut Scene, place: &Place, plugin: &EqPlugin) {
    const STEPS: usize = 216;
    let mut ys = [0.0f32; STEPS + 1];
    for (step, y) in ys.iter_mut().enumerate() {
        let t = step as f32 / STEPS as f32;
        let freq = 20.0 * (20000.0_f32 / 20.0).powf(t);
        *y = gain_y(plugin.magnitude_db(freq));
    }
    let zero = gain_y(0.0);
    let strong = rgba(ACCENT, 0.26);
    let weak = rgba(ACCENT, 0.02);
    let left = dx(place, GRAPH.x);
    let span = ds(place, GRAPH.w).max(1.0);
    for column in left.round() as i32..(left + span).round() as i32 {
        let at = ((column as f32 + 0.5 - left) / span).clamp(0.0, 1.0) * STEPS as f32;
        let index = (at as usize).min(STEPS - 1);
        let y = ys[index] + (ys[index + 1] - ys[index]) * (at - index as f32);
        let (top, bottom) = if y < zero { (strong, weak) } else { (weak, strong) };
        let (from, to) = (y.min(zero), y.max(zero));
        if to - from < 0.5 {
            continue;
        }
        let rect = GlRect::new(column as f32, dy(place, from), 1.0, ds(place, to - from));
        scene.fill_rect(rect, top, bottom, true);
    }
    let step_w = GRAPH.w / STEPS as f32;
    for step in 0..STEPS {
        let x = GRAPH.x + step as f32 * step_w;
        rule(scene, place, x, ys[step], x + step_w, ys[step + 1], rgb(CURVE), 2.2);
    }
}

/// Where a band's node sits. A pass filter has no gain, so its node rides the curve.
fn node_position(plugin: &EqPlugin, band: usize) -> (f32, f32) {
    let freq = value(plugin, freq_index(band));
    let db = if is_pass(kind_of(plugin, band)) {
        plugin.magnitude_db(freq)
    } else {
        value(plugin, gain_index(band))
    };
    (freq_x(freq), gain_y(db))
}

fn meter_y(db: f32) -> f32 {
    let t = (METER_TOP_DB - db.clamp(METER_BOTTOM_DB, METER_TOP_DB)) / (METER_TOP_DB - METER_BOTTOM_DB);
    METER_TRACK_Y + t * METER_TRACK_H
}

/// Green to -12 dB, green into yellow up to 0 dB, orange into red above it.
const METER_ZONES: [(f32, f32, u32, u32); 3] = [
    (METER_BOTTOM_DB, -12.0, 0x1FA97A, 0x4ADE80),
    (-12.0, 0.0, 0x4ADE80, 0xFACC15),
    (0.0, METER_TOP_DB, 0xFB923C, 0xEF4444),
];

fn meter_zones(scene: &mut Scene, place: &Place, x: f32, alpha: f32) {
    for (low, high, bottom, top) in METER_ZONES {
        let zone = Rect { x, y: meter_y(high), w: METER_BAR_W, h: meter_y(low) - meter_y(high) };
        scene.fill_rect(device(place, zone), rgba(top, alpha), rgba(bottom, alpha), true);
    }
}

fn meter_bar(scene: &mut Scene, place: &Place, x: f32, reading: MeterReading) {
    let track = Rect { x, y: METER_TRACK_Y, w: METER_BAR_W, h: METER_TRACK_H };
    round(scene, place, track, 4.0, rgb(WELL), rgb(WELL));
    scene.clip_round(device(place, track), ds(place, 4.0));
    meter_zones(scene, place, x, 0.10);
    if reading.level_db > METER_BOTTOM_DB {
        let top = meter_y(reading.level_db);
        let lit = Rect { x, y: top, w: METER_BAR_W, h: track.bottom() - top };
        scene.clip_round(device(place, lit), 0.0);
        meter_zones(scene, place, x, 1.0);
        scene.pop_clip();
    }
    if reading.hold_db > METER_BOTTOM_DB + 0.5 {
        let color = if reading.hold_db > 0.0 { rgb(CLIP_RED) } else { rgb(INK) };
        let mark = Rect { x, y: meter_y(reading.hold_db) - 1.0, w: METER_BAR_W, h: 2.0 };
        scene.fill_rect(device(place, mark), color, color, true);
    }
    scene.pop_clip();
    outline(scene, place, track, 4.0, rgb(WELL_EDGE));
}

/// Two bars, a title, and the higher of the two held peaks as a number.
fn meter_pair(scene: &mut Scene, place: &Place, cx: f32, title: &str, readings: [MeterReading; 2]) {
    text(scene, place, title, cx, 72.0, 10.5, rgb(MUTED), true);
    let held = readings[0].hold_db.max(readings[1].hold_db);
    let (label, color) = if held <= METER_BOTTOM_DB {
        ("-\u{221e}".to_string(), rgb(FAINT))
    } else if held > 0.0 {
        (format!("+{held:.1}"), rgb(CLIP_RED))
    } else {
        (format!("{:.1}", shown(held)), rgb(INK))
    };
    text(scene, place, label, cx, 92.0, 13.0, color, true);
    let gap = 5.0;
    let lefts = [cx - gap * 0.5 - METER_BAR_W, cx + gap * 0.5];
    for (channel, left) in lefts.into_iter().enumerate() {
        meter_bar(scene, place, left, readings[channel]);
        let letter = if channel == 0 { "L" } else { "R" };
        text(scene, place, letter, left + METER_BAR_W * 0.5, 401.0, 9.0, rgb(FAINT), true);
    }
}

fn draw_meters(scene: &mut Scene, place: &Place, plugin: &EqPlugin) {
    panel(scene, place, METERS, 14.0);
    let input_cx = METERS.x + 46.0;
    let output_cx = METERS.right() - 46.0;
    meter_pair(scene, place, input_cx, "IN", plugin.input_meter());
    meter_pair(scene, place, output_cx, "OUT", plugin.output_meter());
    let scale_cx = METERS.cx();
    for db in [6.0, 0.0, -6.0, -12.0, -24.0, -36.0, -48.0, -60.0] {
        let y = meter_y(db);
        let label = if db > 0.0 { format!("+{db:.0}") } else { format!("{db:.0}") };
        let color = if db == 0.0 { rgb(INK) } else { rgb(MUTED) };
        text(scene, place, label, scale_cx, y, 9.5, color, true);
        for side in [-1.0, 1.0] {
            let near = scale_cx + side * 17.0;
            rule(scene, place, near, y, near + side * 6.0, y, rgba(FAINT, 0.8), 1.0);
        }
    }
}

const PHASE_LABELS: [&str; 3] = ["MP", "LP", "ZL"];
const PHASE_NAMES: [&str; 3] = ["Minimum phase", "Linear phase", "Analog ZL"];

/// Output trim and the phase mode, under the meters.
fn draw_output(scene: &mut Scene, place: &Place, plugin: &EqPlugin) {
    panel(scene, place, OUTPUT, 12.0);
    let cx = OUTPUT.cx();
    text(scene, place, "TRIM", cx, 450.0, 10.5, rgb(MUTED), true);
    well(scene, place, TRIM_BOX, 9.0);
    text(scene, place, gain_label(value(plugin, TRIM_INDEX)), cx, TRIM_BOX.cy(), 15.0, rgb(INK), true);

    text(scene, place, "PHASE", cx, 522.0, 10.5, rgb(MUTED), true);
    let selected = value(plugin, PHASE_INDEX).round().clamp(0.0, 2.0) as usize;
    for mode in 0..3 {
        let button = phase_button(mode);
        if mode == selected {
            round(scene, place, button, 9.0, rgb(ACCENT), rgb(ACCENT_DEEP));
            text(scene, place, PHASE_LABELS[mode], button.cx(), button.cy(), 13.0, rgb(DARK_INK), true);
        } else {
            well(scene, place, button, 9.0);
            text(scene, place, PHASE_LABELS[mode], button.cx(), button.cy(), 13.0, rgb(MUTED), true);
        }
    }
    let latency = plugin.latency();
    let delay = if latency == 0 { "no latency".to_string() } else { format!("{latency} samples") };
    let caption = format!("{} \u{b7} {delay}", PHASE_NAMES[selected]);
    text(scene, place, caption, cx, 592.0, 10.5, rgb(FAINT), true);
}

fn gain_label(db: f32) -> String {
    let db = shown(db);
    if db == 0.0 { "0.0 dB".to_string() } else { format!("{db:+.1} dB") }
}

fn freq_label(hz: f32) -> String {
    let hz = hz.clamp(20.0, 20000.0);
    if hz < 999.5 {
        format!("{hz:.0} Hz")
    } else if hz < 9995.0 {
        format!("{:.2} kHz", hz / 1000.0)
    } else {
        format!("{:.1} kHz", hz / 1000.0)
    }
}

fn draw_bands(scene: &mut Scene, place: &Place, plugin: &EqPlugin) {
    for band in 0..BANDS {
        let card = card(band);
        let color = band_color(band);
        let kind = kind_of(plugin, band);
        let gain = value(plugin, gain_index(band));
        panel(scene, place, card, 12.0);

        let button = type_button(band);
        let badge_cx = card.x + 23.0;
        circle(scene, place, badge_cx, button.cy(), 11.0, color);
        text(scene, place, format!("{}", band + 1), badge_cx, button.cy(), 12.5, rgb(DARK_INK), true);
        well(scene, place, button, 9.0);
        let sign = if gain < 0.0 { -1.0 } else { 1.0 };
        stroke_shape(scene, place, kind, sign, button.cx(), button.cy(), 40.0, 17.0, color, 2.0);

        let rows = [
            ("GAIN", gain_label(gain)),
            ("FREQ", freq_label(value(plugin, freq_index(band)))),
            ("Q", format!("{:.2}", value(plugin, q_index(band)))),
        ];
        for (row, (name, label)) in rows.into_iter().enumerate() {
            let slot = value_box(band, row);
            // A pass filter ignores gain, so that row is dimmed.
            let live = !(row == 0 && is_pass(kind));
            text(scene, place, name, card.x + 12.0, slot.cy(), 9.5, rgb(if live { MUTED } else { FAINT }), false);
            well(scene, place, slot, 8.0);
            text(scene, place, label, slot.cx(), slot.cy(), 12.0, rgb(if live { INK } else { FAINT }), true);
        }
    }
}

/// Height of a filter shape at `u`, both in a unit box: `u` runs 0..1 left to right and the
/// result runs -1 (bottom) to 1 (top). `sign` is -1 when the band cuts, so a peak or shelf
/// symbol dips instead of rising.
fn shape_height(kind: usize, sign: f32, u: f32) -> f32 {
    let step = |u: f32| 1.0 / (1.0 + (-(u - 0.5) * 11.0).exp());
    let roll_off = |u: f32| {
        // Second-order high pass with its corner a third of the way across.
        let ratio = 2.0_f32.powf((u - 0.34) * 7.0);
        let db = 20.0 * (ratio * ratio / (1.0 + ratio.powi(4)).sqrt()).log10();
        0.55 + db.max(-26.0) / 26.0 * 1.45
    };
    match kind {
        1 => roll_off(u),
        2 => roll_off(1.0 - u),
        3 => sign * 0.62 * (1.0 - 2.0 * step(u)),
        4 => sign * 0.62 * (2.0 * step(u) - 1.0),
        _ => sign * (1.35 * (-((u - 0.5) / 0.17).powi(2)).exp() - 0.5),
    }
}

/// Strokes the symbol for a filter type, centred on (`cx`, `cy`) in a `w`×`h` box.
fn stroke_shape(scene: &mut Scene, place: &Place, kind: usize, sign: f32, cx: f32, cy: f32, w: f32, h: f32, color: Color, width: f32) {
    const POINTS: usize = 28;
    let points = (0..=POINTS)
        .map(|index| {
            let u = index as f32 / POINTS as f32;
            let v = shape_height(kind, sign, u).clamp(-1.0, 1.0);
            point(place, cx - w * 0.5 + u * w, cy - v * h * 0.5)
        })
        .collect();
    scene.stroke_poly(points, color, ds(place, width), false);
}

fn node_at(plugin: &EqPlugin, x: f32, y: f32) -> Option<usize> {
    (0..BANDS).rev().find(|&band| {
        let (nx, ny) = node_position(plugin, band);
        let (dx, dy) = (x - nx, y - ny);
        dx * dx + dy * dy < 14.0 * 14.0
    })
}

fn value_at(x: f32, y: f32) -> Option<(usize, usize)> {
    if contains(TRIM_BOX, x, y) {
        return Some((BANDS, 0));
    }
    for band in 0..BANDS {
        for row in 0..3 {
            if contains(value_box(band, row), x, y) {
                return Some((band, row));
            }
        }
    }
    None
}

fn x_to_freq(x: f32) -> f32 {
    let t = ((x - GRAPH.x) / GRAPH.w).clamp(0.0, 1.0);
    20.0 * (20000.0_f32 / 20.0).powf(t)
}

fn y_to_db(y: f32) -> f32 {
    let t = ((y - GRAPH.y) / GRAPH.h).clamp(0.0, 1.0);
    18.0 - t * 36.0
}

fn freq_x(freq: f32) -> f32 {
    let t = (freq.max(20.0) / 20.0).ln() / (20000.0_f32 / 20.0).ln();
    GRAPH.x + t.clamp(0.0, 1.0) * GRAPH.w
}

fn gain_y(db: f32) -> f32 {
    let t = (18.0 - db.clamp(-18.0, 18.0)) / 36.0;
    GRAPH.y + t * GRAPH.h
}
