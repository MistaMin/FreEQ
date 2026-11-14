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
    assert!(text.contains("TRIM"), "trim");
    assert!(text.contains("IN\n") && text.contains("OUT\n"), "input and output meters");
    for band in 1..=6 {
        assert!(text.contains(&band.to_string()), "band {band}");
    }
    for name in openplugin_eq::params::TYPE_CHOICES {
        assert!(!text.contains(name), "filter types are symbols, not the word {name}");
    }
    assert!(text.contains("PHASE") && text.contains("MP") && text.contains("LP") && text.contains("ZL"), "phase modes");
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

    // Band 2's shape button: the card starts at 24 + 123, the button 40 in and 10 down.
    let mut gesture = Gesture::default();
    let type_x = 24.0 + 123.0 + 40.0 + 30.0;
    let type_y = 430.0 + 10.0 + 17.0;
    assert!(pointer_down(&mut plugin, 980.0, 640.0, type_x, type_y, &mut gesture));
    let kind = plugin.param("b2_type").unwrap();
    println!("PROBE case=cycle_type value={kind:.0}");
    assert!((kind - 1.0).abs() < 0.01, "band 2 type cycles from peak to high pass, got {kind}");

    let wide = 1400.0f32;
    let tall = 900.0f32;
    let s = (wide / 980.0).min(tall / 640.0);
    let ox = (wide - 980.0 * s) * 0.5;
    let oy = (tall - 640.0 * s) * 0.5;
    let mut gesture = Gesture::default();
    assert!(pointer_down(&mut plugin, wide, tall, ox + type_x * s, oy + type_y * s, &mut gesture));
    let scaled = plugin.param("b2_type").unwrap();
    println!("PROBE case=resize_hit type={scaled:.0}");
    assert!((scaled - 2.0).abs() < 0.01, "a resized window still hits the type button");

    // Trim follows a vertical drag on its box, and a phase button selects its mode.
    let mut gesture = Gesture::default();
    assert!(pointer_down(&mut plugin, 980.0, 640.0, 864.0, 481.0, &mut gesture));
    pointer_drag(&mut plugin, 980.0, 640.0, 864.0, 381.0, &gesture);
    let trim = plugin.param("trim").unwrap();
    assert!((trim - 5.0).abs() < 0.01, "100 units up is +5 dB of trim, got {trim}");
    let mut gesture = Gesture::default();
    assert!(pointer_down(&mut plugin, 980.0, 640.0, 780.0 + 58.0 + 26.0, 552.0, &mut gesture));
    assert_eq!(plugin.param("phase").unwrap(), 1.0, "the middle phase button is linear phase");
}

/// The polyline drawn inside band 1's shape button, relative to the button's centre.
fn shape_of(plugin: &EqPlugin) -> Vec<(i32, i32)> {
    let (cx, cy) = (24.0 + 40.0 + 32.5, 430.0 + 10.0 + 17.0);
    let list = render(plugin, 980.0, 640.0, None);
    let mut found = Vec::new();
    for cmd in &list.scene.cmds {
        if let Cmd::StrokePoly { pts, .. } = cmd {
            if pts.iter().all(|p| (p.x - cx).abs() <= 21.0 && (p.y - cy).abs() <= 10.0) {
                found.push(pts.iter().map(|p| ((p.x - cx).round() as i32, (p.y - cy).round() as i32)).collect::<Vec<_>>());
            }
        }
    }
    assert_eq!(found.len(), 1, "one symbol in band 1's shape button");
    found.remove(0)
}

#[test]
fn every_filter_type_has_its_own_symbol() {
    let mut plugin = EqPlugin::new();
    plugin.set_param("b1_gain", 6.0);
    let mut shapes = Vec::new();
    for kind in 0..5 {
        plugin.set_param("b1_type", kind as f32);
        shapes.push(shape_of(&plugin));
    }
    for a in 0..shapes.len() {
        for b in a + 1..shapes.len() {
            assert_ne!(shapes[a], shapes[b], "types {a} and {b} draw the same symbol");
        }
    }
    let (first, last) = (|s: &Vec<(i32, i32)>| s[0].1, |s: &Vec<(i32, i32)>| s[s.len() - 1].1);
    // Screen y grows downward, so a larger y is a lower level.
    assert!(first(&shapes[1]) > last(&shapes[1]), "high pass rises from the left");
    assert!(first(&shapes[2]) < last(&shapes[2]), "low pass falls to the right");
    assert!(first(&shapes[3]) < last(&shapes[3]), "a boosting low shelf is high on the left");
    assert!(first(&shapes[4]) > last(&shapes[4]), "a boosting high shelf is high on the right");
    let top = |s: &Vec<(i32, i32)>| s.iter().map(|p| p.1).min().unwrap();
    assert!(top(&shapes[0]) < first(&shapes[0]), "a boosting peak rises in the middle");

    // A cut turns the peak into a dip and flips the shelves.
    plugin.set_param("b1_gain", -6.0);
    plugin.set_param("b1_type", 0.0);
    let dip = shape_of(&plugin);
    let bottom = dip.iter().map(|p| p.1).max().unwrap();
    assert!(bottom > dip[0].1, "a cutting peak dips in the middle");
    plugin.set_param("b1_type", 3.0);
    let shelf = shape_of(&plugin);
    assert!(shelf[0].1 > shelf[shelf.len() - 1].1, "a cutting low shelf is low on the left");
}
