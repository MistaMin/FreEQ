//! A click on empty plot steps the FFT: live, frozen, off, live.

use goodlookindri_shell::Framebuffer;
use openplugin::dri::goodlookindri::color::Color;
use openplugin::dri::goodlookindri::scene::Cmd;
use openplugin::dri::goodlookindri::spectrum::SampleRing;
use openplugin::prelude::Plugin;
use openplugin_eq::{pointer_down, render, spectrum_view, EqPlugin, Gesture, Spectrum, SpectrumMode};

/// Empty plot: 200 Hz at -9 dB, away from every default node and from the curve.
const EMPTY: (f32, f32) = (281.0, 304.0);

fn feed(plugin: &mut EqPlugin, ring: &SampleRing, step: f32) {
    let mut io = vec![vec![0.0_f32; 512]; 2];
    for block in 0..8 {
        for i in 0..512 {
            io[0][i] = ((block * 512 + i) as f32 * step).sin() * 0.25;
            io[1][i] = io[0][i];
        }
        plugin.process(&mut io, 512);
        ring.push(&[&io[0], &io[1]]);
    }
}

/// The words on the editor, and whether the analyser painted.
fn drawn(plugin: &EqPlugin, spectrum: &mut Spectrum) -> (String, bool) {
    let list = render(plugin, 980.0, 640.0, Some(spectrum));
    let mut words = String::new();
    let mut analyser = false;
    for cmd in &list.scene.cmds {
        match cmd {
            Cmd::Text { text, .. } => {
                words.push_str(text);
                words.push('\n');
            }
            // The meters clip to narrow tracks; only the analyser clips to the whole graph.
            Cmd::ClipRound { rect, .. } if rect.w > 600.0 => analyser = true,
            _ => {}
        }
    }
    (words, analyser)
}

#[test]
fn a_click_on_empty_plot_is_reported_and_a_click_on_a_node_is_not() {
    let mut plugin = EqPlugin::new();
    let mut gesture = Gesture::default();
    assert!(pointer_down(&mut plugin, 980.0, 640.0, EMPTY.0, EMPTY.1, &mut gesture));
    assert!(gesture.on_plot(), "empty plot");

    // Band 3's node: 500 Hz, +7.9 dB.
    let node_x = 66.0 + (500.0_f32 / 20.0).ln() / (1000.0_f32).ln() * 646.0;
    let node_y = 82.0 + ((18.0 - 7.9) / 36.0) * 296.0;
    assert!(pointer_down(&mut plugin, 980.0, 640.0, node_x, node_y, &mut gesture));
    assert!(!gesture.on_plot(), "a node is not empty plot");

    // A band card, and the gutter where the dB labels sit, are not the plot either.
    assert!(pointer_down(&mut plugin, 980.0, 640.0, 24.0 + 46.0 + 20.0, 430.0 + 58.0 + 14.0, &mut gesture));
    assert!(!gesture.on_plot(), "a value box");
    assert!(!pointer_down(&mut plugin, 980.0, 640.0, 40.0, 200.0, &mut gesture));
    assert!(!gesture.on_plot(), "the label gutter");

    // The same spot in a resized window.
    let (wide, tall) = (1400.0f32, 900.0f32);
    let s = (wide / 980.0).min(tall / 640.0);
    let (ox, oy) = ((wide - 980.0 * s) * 0.5, (tall - 640.0 * s) * 0.5);
    assert!(pointer_down(&mut plugin, wide, tall, ox + EMPTY.0 * s, oy + EMPTY.1 * s, &mut gesture));
    assert!(gesture.on_plot(), "empty plot in a resized window");

    // No parameter moved.
    let fresh = EqPlugin::new();
    assert_eq!(plugin.values(), fresh.values());
}

#[test]
fn three_clicks_go_live_frozen_off_and_back() {
    let mut spectrum = spectrum_view();
    assert_eq!(spectrum.mode(), SpectrumMode::Live);
    assert_eq!(spectrum.next_mode(), SpectrumMode::Frozen);
    assert_eq!(spectrum.next_mode(), SpectrumMode::Off);
    assert_eq!(spectrum.next_mode(), SpectrumMode::Live);
}

#[test]
fn frozen_keeps_its_picture_and_off_draws_none() {
    let mut plugin = EqPlugin::new();
    plugin.prepare(48000.0, 512, 2);
    let ring = SampleRing::new();
    let mut spectrum = spectrum_view();

    feed(&mut plugin, &ring, 0.17);
    spectrum.update(&ring, 1.0 / 60.0, 48000.0);
    let live_frames = spectrum.analyser().frames_analysed();
    let live_db = spectrum.analyser().display_db().to_vec();
    let (words, analyser) = drawn(&plugin, &mut spectrum);
    assert!(live_frames >= 1 && analyser, "live analyses and paints");
    assert!(!words.contains("FFT"), "live needs no label");

    // Frozen: new audio arrives, the picture and the frame count stay.
    spectrum.next_mode();
    feed(&mut plugin, &ring, 0.61);
    spectrum.update(&ring, 1.0 / 60.0, 48000.0);
    assert_eq!(spectrum.analyser().frames_analysed(), live_frames, "frozen runs no FFT");
    assert_eq!(spectrum.analyser().display_db(), &live_db[..], "frozen keeps the picture");
    let (words, analyser) = drawn(&plugin, &mut spectrum);
    assert!(analyser, "frozen still paints");
    assert!(words.contains("FFT FROZEN"), "frozen says so");

    // Off: nothing analysed, nothing painted, the curve and the bands still there.
    spectrum.next_mode();
    spectrum.update(&ring, 1.0 / 60.0, 48000.0);
    assert_eq!(spectrum.analyser().frames_analysed(), live_frames, "off runs no FFT");
    let (words, analyser) = drawn(&plugin, &mut spectrum);
    assert!(!analyser, "off paints no spectrum");
    assert!(words.contains("FFT OFF") && words.contains("TRIM"), "off says so and the rest is drawn");
    let lines = render(&plugin, 980.0, 640.0, Some(&mut spectrum)).scene.cmds.iter().filter(|cmd| matches!(cmd, Cmd::Line { .. })).count();
    assert!(lines > 200, "the EQ curve is still drawn, {lines} lines");

    // Live again: the FFT picks up the audio that arrived meanwhile.
    spectrum.next_mode();
    spectrum.update(&ring, 1.0 / 60.0, 48000.0);
    assert!(spectrum.analyser().frames_analysed() > live_frames, "live again analyses");
    assert_ne!(spectrum.analyser().display_db(), &live_db[..], "and shows the new audio");
    let (words, analyser) = drawn(&plugin, &mut spectrum);
    assert!(analyser && !words.contains("FFT"));

    // Pictures for the eye, when asked for.
    if let Ok(dir) = std::env::var("FREEQ_SHOTS") {
        for mode in [SpectrumMode::Frozen, SpectrumMode::Off] {
            spectrum.set_mode(mode);
            let list = render(&plugin, 980.0, 640.0, Some(&mut spectrum));
            let mut frame = Framebuffer::new(1960, 1280);
            frame.clear(Color(0xff000000));
            frame.replay(&list.scene, 2.0);
            std::fs::write(format!("{dir}/fft-{mode:?}.ppm"), frame.to_ppm()).unwrap();
        }
    }
}
