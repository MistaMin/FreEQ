//! Input and output meters: where they tap the signal, and how the bar and the hold marker move.

use openplugin::prelude::Plugin;
use openplugin_eq::dsp::{METER_FALL_DB_PER_S, METER_FLOOR_DB, METER_HOLD_S};
use openplugin_eq::EqPlugin;

const RATE: f32 = 48000.0;
const BLOCK: usize = 480;
const BLOCK_S: f32 = BLOCK as f32 / RATE;

fn flat_plugin() -> EqPlugin {
    let mut plugin = EqPlugin::new();
    plugin.prepare(RATE, BLOCK, 2);
    for band in 1..=6 {
        plugin.set_param(&format!("b{band}_type"), 0.0);
        plugin.set_param(&format!("b{band}_gain"), 0.0);
    }
    plugin
}

/// Runs `blocks` blocks of a 1 kHz sine at `left` and `right` peak amplitude.
fn run(plugin: &mut EqPlugin, blocks: usize, left: f32, right: f32, phase: &mut usize) {
    let mut io = vec![vec![0.0_f32; BLOCK]; 2];
    for _ in 0..blocks {
        for i in 0..BLOCK {
            let s = (std::f32::consts::TAU * 1000.0 * (*phase + i) as f32 / RATE).sin();
            io[0][i] = s * left;
            io[1][i] = s * right;
        }
        *phase += BLOCK;
        plugin.process(&mut io, BLOCK);
    }
}

#[test]
fn meters_start_silent_and_read_each_channel() {
    let mut plugin = flat_plugin();
    for reading in plugin.input_meter().into_iter().chain(plugin.output_meter()) {
        assert_eq!(reading.level_db, METER_FLOOR_DB);
        assert_eq!(reading.hold_db, METER_FLOOR_DB);
    }
    let mut phase = 0;
    run(&mut plugin, 20, 0.5, 0.25, &mut phase);
    let input = plugin.input_meter();
    let output = plugin.output_meter();
    println!("PROBE case=meter_levels in_l={:.2} in_r={:.2} out_l={:.2} out_r={:.2}", input[0].level_db, input[1].level_db, output[0].level_db, output[1].level_db);
    assert!((input[0].level_db - -6.02).abs() < 0.1, "0.5 peak is -6 dBFS, got {}", input[0].level_db);
    assert!((input[1].level_db - -12.04).abs() < 0.1, "0.25 peak is -12 dBFS, got {}", input[1].level_db);
    assert!((output[0].level_db - input[0].level_db).abs() < 0.1, "a flat EQ passes the level through");
    assert!((output[1].level_db - input[1].level_db).abs() < 0.1);
}

#[test]
fn input_is_before_the_eq_and_output_is_after_trim() {
    let mut plugin = flat_plugin();
    plugin.set_param("b3_freq", 1000.0);
    plugin.set_param("b3_gain", 6.0);
    plugin.set_param("trim", 3.0);
    let mut phase = 0;
    run(&mut plugin, 50, 0.1, 0.1, &mut phase);
    let input = plugin.input_meter()[0].level_db;
    let output = plugin.output_meter()[0].level_db;
    println!("PROBE case=meter_taps in={input:.2} out={output:.2}");
    assert!((input - -20.0).abs() < 0.1, "input ignores the boost and the trim, got {input}");
    assert!((output - -11.0).abs() < 0.3, "output shows +6 dB of EQ and +3 dB of trim, got {output}");
}

#[test]
fn bar_falls_at_the_stated_rate_and_the_marker_holds_first() {
    let mut plugin = flat_plugin();
    let mut phase = 0;
    run(&mut plugin, 20, 0.5, 0.5, &mut phase);
    let peak = plugin.output_meter()[0].level_db;

    // Half a second of silence: the bar has fallen, the marker has not moved.
    let half = (0.5 / BLOCK_S).round() as usize;
    run(&mut plugin, half, 0.0, 0.0, &mut phase);
    let reading = plugin.output_meter()[0];
    let fallen = peak - reading.level_db;
    println!("PROBE case=meter_fall after_s=0.5 fell_db={fallen:.2} hold_db={:.2}", reading.hold_db);
    assert!((fallen - METER_FALL_DB_PER_S * 0.5).abs() < 0.5, "fell {fallen} dB in half a second");
    assert_eq!(reading.hold_db, peak, "the marker holds while the bar falls");
    let input = plugin.input_meter()[0];
    assert!((input.hold_db - -6.02).abs() < 0.1 && input.level_db < input.hold_db - 9.0, "the input meter moves the same way");

    // Past the hold time the marker falls as well, and never ends up under the bar.
    let rest = ((METER_HOLD_S + 0.5 - 0.5) / BLOCK_S).round() as usize;
    run(&mut plugin, rest, 0.0, 0.0, &mut phase);
    let reading = plugin.output_meter()[0];
    println!("PROBE case=meter_hold after_s={:.1} level_db={:.2} hold_db={:.2}", METER_HOLD_S + 0.5, reading.level_db, reading.hold_db);
    assert!(reading.hold_db < peak - 5.0, "the marker let go after the hold time, at {}", reading.hold_db);
    assert!(reading.hold_db >= reading.level_db, "the marker stays on or above the bar");

    // Long silence: both rest on the floor.
    run(&mut plugin, (6.0 / BLOCK_S) as usize, 0.0, 0.0, &mut phase);
    let reading = plugin.output_meter()[0];
    assert_eq!(reading.level_db, METER_FLOOR_DB);
    assert_eq!(reading.hold_db, METER_FLOOR_DB);

    // A louder block lifts the bar and the marker at once.
    run(&mut plugin, 1, 0.9, 0.9, &mut phase);
    let reading = plugin.output_meter()[0];
    assert!(reading.level_db > -1.5 && reading.hold_db == reading.level_db, "attack is immediate");
}

#[test]
fn fall_rate_does_not_depend_on_block_size() {
    let mut fell = Vec::new();
    for block in [64usize, 480, 2048] {
        let mut plugin = flat_plugin();
        let mut io = vec![vec![0.5_f32; block]; 2];
        plugin.process(&mut io, block);
        let peak = plugin.output_meter()[0].level_db;
        let blocks = (RATE * 0.5 / block as f32).round() as usize;
        for _ in 0..blocks {
            io[0].fill(0.0);
            io[1].fill(0.0);
            plugin.process(&mut io, block);
        }
        let seconds = blocks as f32 * block as f32 / RATE;
        fell.push((peak - plugin.output_meter()[0].level_db) / seconds);
    }
    println!("PROBE case=meter_block_size rates={fell:?}");
    for rate in fell {
        assert!((rate - METER_FALL_DB_PER_S).abs() < 0.2, "fall rate {rate} dB/s");
    }
}
