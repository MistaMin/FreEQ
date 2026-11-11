//! OpenPlugin trait adapter. Audio-thread `process` only calls `EqDsp::process_sample`.

use openplugin::prelude::*;

use crate::dsp::EqDsp;
use crate::params::{PARAM_COUNT, PARAMS};

static INFO: PluginInfo = PluginInfo {
    id: "com.mistamin.eqbenchmark.openplugin",
    name: "EQ Benchmark OpenPlugin",
    vendor: "MistaMin",
    url: "https://github.com/MistaMin/OpenPluginSDK",
    email: "contact@mistamin.com",
    version: "0.1.0",
    category: Category::Eq,
    params: &PARAMS,
    is_instrument: false,
    has_editor: true,
};

pub struct EqPlugin {
    values: [f32; PARAM_COUNT],
    dsp: EqDsp,
}

impl EqPlugin {
    pub fn new() -> Self {
        let mut values = [0.0; PARAM_COUNT];
        for (index, param) in PARAMS.iter().enumerate() {
            values[index] = param.default;
        }
        let mut plugin = Self {
            values,
            dsp: EqDsp::new(),
        };
        plugin.dsp.prepare(48000.0, 2);
        plugin.dsp.set_params(&plugin.values);
        plugin
    }

    pub fn values(&self) -> &[f32; PARAM_COUNT] {
        &self.values
    }

    pub fn spectrum(&self) -> &[f32; crate::dsp::SPECTRUM_BINS] {
        self.dsp.spectrum()
    }

    pub fn magnitude_db(&self, hz: f32) -> f32 {
        self.dsp.magnitude_db(hz)
    }

    pub fn output_db(&self) -> [f32; 2] {
        self.dsp.output_db()
    }

    pub fn sample_rate(&self) -> f32 {
        self.dsp.sample_rate()
    }

    /// In-place stereo block. The window's audio callback uses this so it does not build a `Vec` per buffer.
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        self.dsp.process_channel(0, &mut left[..frames]);
        self.dsp.process_channel(1, &mut right[..frames]);
    }
}

impl Default for EqPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EqPlugin {
    fn info(&self) -> &'static PluginInfo {
        &INFO
    }

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.dsp.prepare(sample_rate, channels);
        self.dsp.set_params(&self.values);
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn set_param(&mut self, id: &str, value: f32) -> bool {
        let Some(index) = PARAMS.iter().position(|param| param.id == id) else {
            return false;
        };
        self.values[index] = PARAMS[index].clamp(value);
        self.dsp.set_params(&self.values);
        true
    }

    fn param(&self, id: &str) -> Option<f32> {
        PARAMS
            .iter()
            .position(|param| param.id == id)
            .map(|index| self.values[index])
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let channels = io.len().min(8);
        for channel in 0..channels {
            let available = io[channel].len();
            let count = frames.min(available);
            self.dsp.process_channel(channel, &mut io[channel][..count]);
        }
    }
}
