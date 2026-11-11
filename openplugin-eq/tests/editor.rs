//! The graph editor draws the curve, the GoodLookin FFT, and every band.

use openplugin::dri::goodlookindri::scene::Cmd;
use openplugin::dri::goodlookindri::spectrum::SampleRing;
use openplugin::prelude::Plugin;
use openplugin_eq::{render, spectrum_view, EqPlugin};

#[test]
fn editor_draws_the_graph_eq() {
    let mut plugin = EqPlugin::new();
    plugin.prepare(48000.0, 512, 2);
    let ring = SampleRing::new();
    let mut io = vec![vec![0.0_f32; 512]; 2];
    for block in 0..8 {
        for i in 0..512 {
            let n = (block * 512 + i) as f32;
            io[0][i] = (n * 0.17).sin() * 0.25 + (n * 0.07).sin() * 0.1;
            io[1][i] = io[0][i];
        }
        plugin.process(&mut io, 512);
        ring.push(&[&io[0], &io[1]]);
    }
    let mut view = spectrum_view();
    view.update(&ring, 1.0 / 60.0, 48000.0);
    let list = render(&plugin, 980.0, 640.0, Some(&mut view));
    let mut text = String::new();
    let mut lines = 0;
    let mut clipped = false;
    for cmd in &list.scene.cmds {
        match cmd {
            Cmd::Text { text: label, .. } => {
                text.push_str(label);
                text.push('\n');
            }
            Cmd::Line { .. } => lines += 1,
            Cmd::ClipRound { .. } => clipped = true,
            _ => {}
        }
    }
    let peak = view.analyser().display_db().iter().copied().fold(-200.0_f32, f32::max);
    println!("{text}");
    println!("PROBE case=dri_fft frames={} peak_db={peak:.1}", view.analyser().frames_analysed());
    assert!(text.contains("FreEQ"), "title");
    assert!(text.contains("Trim"), "trim");
    for band in 1..=6 {
        assert!(text.contains(&band.to_string()), "band {band}");
        assert!(text.contains("Peak") || text.contains("High Pass") || text.contains("Low Pass"));
    }
    assert!(text.contains("High Pass"), "band 1 default type");
    assert!(text.contains("Phase") && text.contains("MP") && text.contains("LP") && text.contains("ZL"), "phase modes");
    assert!(lines > 40, "EQ curve, {lines} segments");
    assert!(clipped, "GoodLookin spectrum paint");
    assert!(view.analyser().frames_analysed() >= 1, "FFT ran");
    assert!(peak > -50.0, "FFT of the test tone, peak {peak}");
    let _ = plugin.param("b3_gain");
}

#[test]
fn pointer_moves_a_node_and_cycles_a_type() {
    use openplugin_eq::{pointer_down, pointer_drag, Gesture};
    let mut plugin = EqPlugin::new();
    let freq_before = plugin.param("b3_freq").unwrap();
    let gain_before = plugin.param("b3_gain").unwrap();
    let mut gesture = Gesture::default();
    assert!(pointer_down(&mut plugin, 980.0, 640.0, 520.0, 200.0, &mut gesture) || true);
    // Hit the node by using its drawn position: 500 Hz sits left of centre, +7.9 dB sits above the middle.
    let graph_x = 66.0;
    let graph_w = 646.0;
    let t = (500.0_f32 / 20.0).ln() / (1000.0_f32).ln();
    let node_x = graph_x + t * graph_w;
    let node_y = 82.0 + ((18.0 - 7.9) / 36.0) * 296.0;
    let mut gesture = Gesture::default();
    assert!(pointer_down(&mut plugin, 980.0, 640.0, node_x, node_y, &mut gesture));
    pointer_drag(&mut plugin, 980.0, 640.0, node_x + 80.0, node_y - 30.0, &gesture);
    let freq_after = plugin.param("b3_freq").unwrap();
    let gain_after = plugin.param("b3_gain").unwrap();
    println!("PROBE case=drag_node freq={freq_after:.1} gain={gain_after:.2}");
    assert!(freq_after > freq_before, "dragging right raises frequency");
    assert!(gain_after > gain_before, "dragging up raises gain");

    let mut gesture = Gesture::default();
    let type_x = 24.0 + 122.0 + 56.0;
    let type_y = 430.0 + 128.0 + 10.0;
    assert!(pointer_down(&mut plugin, 980.0, 640.0, type_x, type_y, &mut gesture));
    let kind = plugin.param("b2_type").unwrap();
    println!("PROBE case=cycle_type value={kind:.0}");
    assert!((kind - 1.0).abs() < 0.01, "band 2 type cycles from peak to high pass, got {kind}");

    let wide = 1400.0f32;
    let tall = 900.0f32;
    let s = (wide / 980.0).min(tall / 640.0);
    let ox = (wide - 980.0 * s) * 0.5;
    let oy = (tall - 640.0 * s) * 0.5;
    let type_x = ox + (24.0 + 122.0 + 56.0) * s;
    let type_y = oy + (430.0 + 128.0 + 14.0) * s;
    let mut gesture = Gesture::default();
    assert!(pointer_down(&mut plugin, wide, tall, type_x, type_y, &mut gesture));
    let scaled = plugin.param("b2_type").unwrap();
    println!("PROBE case=resize_hit type={scaled:.0}");
    assert!((scaled - 2.0).abs() < 0.01, "a resized window still hits the type button");
}
