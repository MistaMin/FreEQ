//! Linear phase and Analog ZL still sit on the six-band EQ.

use openplugin::prelude::Plugin;
use openplugin_eq::{pointer_down, EqPlugin, Gesture};

fn select_phase(plugin: &mut EqPlugin, mode: usize) {
    let x = 786.0 + mode as f32 * 64.0 + 28.0;
    let y = 444.0;
    let mut gesture = Gesture::default();
    assert!(pointer_down(plugin, 980.0, 640.0, x, y, &mut gesture), "phase button {mode}");
    let value = plugin.param("phase").unwrap();
    assert!((value - mode as f32).abs() < 0.01, "GUI set phase to {value}, wanted {mode}");
}

const SR: f32 = 48000.0;
const PEAK: f32 = 0.25;

fn fresh() -> EqPlugin {
    let mut plugin = EqPlugin::new();
    plugin.prepare(SR, 512, 2);
    for band in 1..=6 {
        plugin.set_param(&format!("b{band}_type"), 0.0);
        plugin.set_param(&format!("b{band}_gain"), 0.0);
    }
    plugin
}

fn level_db(plugin: &mut EqPlugin, hz: f32) -> f64 {
    plugin.reset();
    let mut io = vec![vec![0.0_f32; 512]; 2];
    let mut sum_sq = 0.0_f64;
    let mut counted = 0_usize;
    let mut produced = 0_usize;
    let mut phase = 0.0_f32;
    let step = hz / SR;
    while counted < 8192 {
        for i in 0..512 {
            phase = (phase + step) % 1.0;
            let sample = (phase * std::f32::consts::TAU).sin() * PEAK;
            io[0][i] = sample;
            io[1][i] = sample;
        }
        plugin.process(&mut io, 512);
        for &sample in &io[0] {
            if produced >= 8192 && counted < 8192 {
                let y = sample as f64;
                sum_sq += y * y;
                counted += 1;
            }
            produced += 1;
        }
    }
    let rms_out = (sum_sq / counted as f64).sqrt();
    let rms_in = (PEAK as f64) / std::f64::consts::SQRT_2;
    if rms_out < 1.0e-12 { -120.0 } else { 20.0 * (rms_out / rms_in).log10() }
}

#[test]
fn linear_and_analog_zl_keep_a_boost() {
    let names = ["MP", "LP", "ZL"];
    let mut levels = [0.0; 3];
    for mode in 0..3 {
        let mut plugin = fresh();
        plugin.set_param("b3_freq", 1000.0);
        plugin.set_param("b3_q", 1.0);
        plugin.set_param("b3_gain", 6.0);
        select_phase(&mut plugin, mode);
        levels[mode] = level_db(&mut plugin, 1000.0);
        let latency = plugin.latency();
        println!("PROBE case=gui_phase mode={} level_db={:.3} latency={latency}", names[mode], levels[mode]);
        let expected = if mode == 1 { 512 } else { 0 };
        assert_eq!(latency, expected, "{} latency", names[mode]);
        assert!((levels[mode] - 6.0).abs() < 1.5, "{} level {}", names[mode], levels[mode]);
    }
}
