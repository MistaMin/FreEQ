// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marcos Quirino Deida
//! CPU time of the shipped OpenPlugin EQ process path. Release build only.

use openplugin::prelude::Plugin;
use openplugin_eq::EqPlugin;
use std::hint::black_box;

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

const CLOCK_PROCESS_CPUTIME_ID: i32 = 12;

extern "C" {
    fn clock_gettime(clock_id: i32, ts: *mut Timespec) -> i32;
}

fn cpu_now() -> f64 {
    let mut ts = Timespec { tv_sec: 0, tv_nsec: 0 };
    let rc = unsafe { clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0);
    ts.tv_sec as f64 + ts.tv_nsec as f64 * 1e-9
}

const SR: f32 = 48000.0;
const BLOCK: usize = 512;
const WARM_BLOCKS: usize = 187;
const MEASURE_BLOCKS: usize = 750;

fn sine(total_blocks: usize) -> Vec<f32> {
    let mut samples = vec![0.0f32; total_blocks * BLOCK];
    let mut phase = 0.0f32;
    let step = 1000.0 / SR;
    for sample in &mut samples {
        phase = (phase + step) % 1.0;
        *sample = (phase * std::f32::consts::TAU).sin() * 0.25;
    }
    samples
}

fn once(case: &str, source: &[f32], mut setup: impl FnMut(&mut EqPlugin)) -> f64 {
    let mut plugin = EqPlugin::new();
    plugin.prepare(SR, BLOCK, 2);
    setup(&mut plugin);
    let mut io = vec![vec![0.0f32; BLOCK]; 2];
    for block in 0..WARM_BLOCKS {
        let start = block * BLOCK;
        io[0].copy_from_slice(&source[start..start + BLOCK]);
        io[1].copy_from_slice(&source[start..start + BLOCK]);
        plugin.process(&mut io, BLOCK);
    }
    let mut sink = 0.0f32;
    let begin = cpu_now();
    for block in 0..MEASURE_BLOCKS {
        let start = (WARM_BLOCKS + block) * BLOCK;
        io[0].copy_from_slice(&source[start..start + BLOCK]);
        io[1].copy_from_slice(&source[start..start + BLOCK]);
        plugin.process(&mut io, BLOCK);
        sink += io[0][0];
    }
    let with_copy = cpu_now() - begin;
    let copy_begin = cpu_now();
    for block in 0..MEASURE_BLOCKS {
        let start = (WARM_BLOCKS + block) * BLOCK;
        io[0].copy_from_slice(&source[start..start + BLOCK]);
        io[1].copy_from_slice(&source[start..start + BLOCK]);
        sink += io[0][0];
    }
    let copy_only = cpu_now() - copy_begin;
    black_box(sink);
    assert!(sink.is_finite(), "{case} output was not finite");
    let audio = (MEASURE_BLOCKS * BLOCK) as f64 / f64::from(SR);
    let cpu = (with_copy - copy_only).max(0.0);
    let pct = cpu / audio * 100.0;
    println!(
        "CPU plugin=openplugin case={case} audio_sec={audio:.3} cpu_sec={cpu:.6} pct_realtime={pct:.4} gross_cpu_sec={with_copy:.6} copy_cpu_sec={copy_only:.6} sink={sink:.6}"
    );
    pct
}

fn unity(plugin: &mut EqPlugin) {
    for band in 1..=6 {
        plugin.set_param(&format!("b{band}_type"), 0.0);
        plugin.set_param(&format!("b{band}_gain"), 0.0);
    }
    plugin.set_param("trim", 0.0);
}

/// One bell. The other five bands stay at 0 dB.
fn one(plugin: &mut EqPlugin) {
    unity(plugin);
    plugin.set_param("b3_freq", 1000.0);
    plugin.set_param("b3_q", 1.0);
    plugin.set_param("b3_gain", 6.0);
}

/// High-pass, low-pass, and four bells.
fn full(plugin: &mut EqPlugin) {
    unity(plugin);
    plugin.set_param("b1_type", 1.0);
    plugin.set_param("b1_freq", 100.0);
    plugin.set_param("b2_type", 2.0);
    plugin.set_param("b2_freq", 8000.0);
    for band in 3..=6 {
        plugin.set_param(&format!("b{band}_gain"), 6.0);
        plugin.set_param(&format!("b{band}_q"), 2.0);
    }
}

/// One hundred independent tracks, each with its own EQ. The percentage is the cost of all of them against one playback of the audio.
fn song(source: &[f32], dsp_name: &str, dsp_value: f32) {
    const INSTANCES: usize = 100;
    let case = format!("song_{dsp_name}");
    let mut samples = Vec::new();
    let mut latency = 0_usize;
    for pass in 0..4 {
        let mut plugins = Vec::with_capacity(INSTANCES);
        for _ in 0..INSTANCES {
            let mut plugin = EqPlugin::new();
            plugin.prepare(SR, BLOCK, 2);
            full(&mut plugin);
            plugin.set_param("phase", dsp_value);
            latency = plugin.latency();
            plugins.push(plugin);
        }
        let mut io = vec![vec![0.0_f32; BLOCK]; 2];
        for block in 0..WARM_BLOCKS {
            let start = block * BLOCK;
            let slice = &source[start..start + BLOCK];
            for plugin in &mut plugins {
                io[0].copy_from_slice(slice);
                io[1].copy_from_slice(slice);
                plugin.process(&mut io, BLOCK);
            }
        }
        let mut sink = 0.0_f32;
        let begin = cpu_now();
        for block in 0..MEASURE_BLOCKS {
            let start = (WARM_BLOCKS + block) * BLOCK;
            let slice = &source[start..start + BLOCK];
            for plugin in &mut plugins {
                io[0].copy_from_slice(slice);
                io[1].copy_from_slice(slice);
                plugin.process(&mut io, BLOCK);
                sink += io[0][0];
            }
        }
        let with_copy = cpu_now() - begin;
        let copy_begin = cpu_now();
        for block in 0..MEASURE_BLOCKS {
            let start = (WARM_BLOCKS + block) * BLOCK;
            let slice = &source[start..start + BLOCK];
            for _ in &plugins {
                io[0].copy_from_slice(slice);
                io[1].copy_from_slice(slice);
                sink += io[0][0];
            }
        }
        let copy_only = cpu_now() - copy_begin;
        black_box(sink);
        assert!(sink.is_finite(), "{case} output was not finite");
        let audio = (MEASURE_BLOCKS * BLOCK) as f64 / f64::from(SR);
        let pct = (with_copy - copy_only).max(0.0) / audio * 100.0;
        if pass > 0 {
            samples.push(pct);
        }
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = samples[samples.len() / 2];
    println!(
        "CPU plugin=openplugin instances={INSTANCES} dsp={dsp_name} load=full fft=off median_pct_realtime={median:.2} latency={latency}"
    );
}

fn main() {
    let source = sine(WARM_BLOCKS + MEASURE_BLOCKS);
    if std::env::args().any(|arg| arg == "--song") {
        for (dsp_name, dsp_value) in [("mp", 0.0_f32), ("lp", 1.0), ("zl", 2.0)] {
            song(&source, dsp_name, dsp_value);
        }
        return;
    }
    let dsps = [("mp", 0.0_f32), ("lp", 1.0), ("zl", 2.0)];
    let loads: [(&str, fn(&mut EqPlugin)); 3] = [("idle", unity), ("one", one), ("full", full)];
    for (dsp_name, dsp_value) in dsps {
        for (load_name, load) in loads {
            let case = format!("{dsp_name}_{load_name}");
            let mut samples = Vec::new();
            let mut latency = 0_usize;
            for pass in 0..6 {
                let pct = once(&case, &source, |plugin| {
                    load(plugin);
                    plugin.set_param("phase", dsp_value);
                    latency = plugin.latency();
                });
                if pass > 0 {
                    samples.push(pct);
                }
            }
            samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = samples[samples.len() / 2];
            println!(
                "CPU plugin=openplugin dsp={dsp_name} load={load_name} fft=off median_pct_realtime={median:.4} latency={latency}"
            );
        }
    }
}
