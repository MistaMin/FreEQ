//! Sine probes through the shipped six-band EQ.

use openplugin::prelude::Plugin;
use openplugin_eq::EqPlugin;

const SR: f32 = 48000.0;
const PEAK: f32 = 0.25;

fn fresh() -> EqPlugin {
    let mut plugin = EqPlugin::new();
    plugin.prepare(SR, 512, 2);
    plugin
}

fn set(plugin: &mut EqPlugin, id: &str, value: f32) {
    assert!(plugin.set_param(id, value), "unknown param {id}");
}

fn flat(plugin: &mut EqPlugin) {
    for band in 1..=6 {
        set(plugin, &format!("b{band}_type"), 0.0);
        set(plugin, &format!("b{band}_gain"), 0.0);
        set(plugin, &format!("b{band}_q"), 0.707);
    }
    set(plugin, "phase", 0.0);
    set(plugin, "trim", 0.0);
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
fn each_filter_type_does_its_job() {
    let mut highpass = fresh();
    flat(&mut highpass);
    set(&mut highpass, "b1_type", 1.0);
    set(&mut highpass, "b1_freq", 400.0);
    let low = level_db(&mut highpass, 80.0);
    let high = level_db(&mut highpass, 2000.0);
    println!("PROBE case=highpass low={low:.3} high={high:.3}");
    assert!(low < high - 12.0, "highpass low {low:.2} high {high:.2}");

    let mut lowpass = fresh();
    flat(&mut lowpass);
    set(&mut lowpass, "b1_type", 2.0);
    set(&mut lowpass, "b1_freq", 1000.0);
    let passed = level_db(&mut lowpass, 200.0);
    let cut = level_db(&mut lowpass, 8000.0);
    println!("PROBE case=lowpass pass={passed:.3} cut={cut:.3}");
    assert!(cut < passed - 18.0, "lowpass");

    let mut peak = fresh();
    flat(&mut peak);
    set(&mut peak, "b3_freq", 1000.0);
    set(&mut peak, "b3_q", 2.0);
    set(&mut peak, "b3_gain", 12.0);
    let boosted = level_db(&mut peak, 1000.0);
    println!("PROBE case=peak level_db={boosted:.3}");
    assert!((boosted - 12.0).abs() < 2.0, "peak {boosted:.2}");

    let mut shelf = fresh();
    flat(&mut shelf);
    set(&mut shelf, "b6_type", 4.0);
    set(&mut shelf, "b6_freq", 4000.0);
    set(&mut shelf, "b6_gain", 6.0);
    let air = level_db(&mut shelf, 12000.0);
    let body = level_db(&mut shelf, 200.0);
    println!("PROBE case=highshelf air={air:.3} body={body:.3}");
    assert!(air > 3.0, "high shelf {air:.2}");
    assert!(body.abs() < 1.5, "shelf lows {body:.2}");

    let mut low_shelf = fresh();
    flat(&mut low_shelf);
    set(&mut low_shelf, "b2_type", 3.0);
    set(&mut low_shelf, "b2_freq", 200.0);
    set(&mut low_shelf, "b2_gain", 6.0);
    let bass = level_db(&mut low_shelf, 60.0);
    println!("PROBE case=lowshelf level_db={bass:.3}");
    assert!(bass > 3.0, "low shelf {bass:.2}");
}

#[test]
fn band_frequency_reaches_both_ends() {
    let mut low = fresh();
    flat(&mut low);
    set(&mut low, "b4_freq", 20.0);
    set(&mut low, "b4_q", 2.0);
    set(&mut low, "b4_gain", 12.0);
    let at20 = level_db(&mut low, 20.0);

    let mut high = fresh();
    flat(&mut high);
    set(&mut high, "b4_freq", 20000.0);
    set(&mut high, "b4_q", 2.0);
    set(&mut high, "b4_gain", 12.0);
    let at20k = level_db(&mut high, 20000.0);
    println!("PROBE case=band_ends at20={at20:.3} at20k={at20k:.3}");
    assert!(at20 > 6.0, "20 Hz peak {at20:.2}");
    assert!(at20k > 4.0, "20 kHz peak {at20k:.2}");
}
