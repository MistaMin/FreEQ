//! Six series bands, each a peak, pass, or shelf. Minimum phase is the biquad
//! path. Linear phase is a symmetric FIR of that magnitude. Analog ZL designs
//! the same sections at 16× and renders a causal kernel with no latency.

use openplugin::dsp::BlockConvolver;
use openplugin::prelude::{Biquad, FilterType};

use crate::params::{
    freq_index, gain_index, q_index, type_index, BANDS, PARAM_COUNT, PHASE_INDEX, TRIM_INDEX,
};

const MAX_CHANNELS: usize = 8;
const FIR_LEN: usize = 1024;
pub const SPECTRUM_BINS: usize = 64;

#[derive(Debug, Clone)]
struct Channel {
    bands: [Biquad; BANDS],
}

impl Channel {
    fn new() -> Self {
        Self {
            bands: std::array::from_fn(|_| Biquad::new()),
        }
    }

    fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
    }
}

#[derive(Debug, Clone)]
pub struct EqDsp {
    sample_rate: f32,
    values: [f32; PARAM_COUNT],
    phase_mode: u8,
    trim_lin: f32,
    fir: [f32; FIR_LEN],
    /// Neon overlap-save from the SDK. Replaces the scalar 2048-point complex transform.
    convolver: BlockConvolver,
    spectrum: [f32; SPECTRUM_BINS],
    meter: [f32; 2],
    channels: Vec<Channel>,
}

impl EqDsp {
    pub fn new() -> Self {
        let mut dsp = Self {
            sample_rate: 48000.0,
            values: defaults(),
            phase_mode: 0,
            trim_lin: 1.0,
            fir: identity_fir(),
            convolver: BlockConvolver::new(1),
            spectrum: [-100.0; SPECTRUM_BINS],
            meter: [-80.0; 2],
            channels: Vec::new(),
        };
        dsp.apply_coeffs();
        dsp
    }

    pub fn prepare(&mut self, sample_rate: f32, channels: usize) {
        self.sample_rate = sample_rate.max(1000.0);
        let n = channels.clamp(1, MAX_CHANNELS);
        self.channels.clear();
        self.channels.resize(n, Channel::new());
        self.apply_coeffs();
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        self.convolver.reset();
        self.meter = [-80.0; 2];
    }

    pub fn latency(&self) -> usize {
        if self.phase_mode == 1 { FIR_LEN / 2 } else { 0 }
    }

    pub fn spectrum(&self) -> &[f32; SPECTRUM_BINS] {
        &self.spectrum
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn output_db(&self) -> [f32; 2] {
        self.meter
    }

    pub fn set_params(&mut self, values: &[f32; PARAM_COUNT]) {
        self.values = *values;
        self.apply_coeffs();
    }

    /// Magnitude of the minimum-phase biquad chain, in dB, including trim.
    pub fn magnitude_db(&self, hz: f32) -> f32 {
        let mut hr = 1.0f32;
        let mut hi = 0.0f32;
        for stage in host_stages(&self.values, self.sample_rate) {
            let (sr, si) = stage_response(&stage, hz, self.sample_rate);
            let nr = hr * sr - hi * si;
            hi = hr * si + hi * sr;
            hr = nr;
        }
        let mag = (hr * hr + hi * hi).sqrt().max(1.0e-8);
        20.0 * mag.log10() + self.values[TRIM_INDEX]
    }

    pub fn process_sample(&mut self, channel: usize, x: f32) -> f32 {
        let mut sample = [x];
        self.process_channel(channel, &mut sample);
        sample[0]
    }

    /// In-place block. FIR modes use one 2048-point overlap-save per chunk of at most 1024 samples.
    /// Minimum phase skips a peak whose gain is 0 dB, and updates the meter once per block.
    pub fn process_channel(&mut self, channel: usize, samples: &mut [f32]) {
        if self.channels.get(channel).is_none() || samples.is_empty() {
            return;
        }
        if self.phase_mode == 0 {
            self.process_minimum(channel, samples);
        } else {
            self.process_fir(channel, samples);
        }
    }

    fn process_minimum(&mut self, channel: usize, samples: &mut [f32]) {
        let trim = self.trim_lin;
        let mut active = [true; BANDS];
        for band in 0..BANDS {
            let kind = filter_kind(self.values[type_index(band)]);
            let gain = self.values[gain_index(band)];
            active[band] = !(matches!(kind, FilterType::Peaking) && gain.abs() < 1.0e-3);
        }
        let state = &mut self.channels[channel];
        let mut peak = 0.0f32;
        for sample in samples.iter_mut() {
            let mut y = *sample;
            for (band, on) in state.bands.iter_mut().zip(active) {
                if on {
                    y = band.process_sample(y);
                }
            }
            y *= trim;
            peak = peak.max(y.abs());
            *sample = y;
        }
        self.push_meter(channel, peak);
    }

    fn process_fir(&mut self, channel: usize, samples: &mut [f32]) {
        self.convolver.process(channel, samples);
        let trim = self.trim_lin;
        let mut peak = 0.0f32;
        for sample in samples.iter_mut() {
            *sample *= trim;
            peak = peak.max(sample.abs());
        }
        self.push_meter(channel, peak);
    }

    fn push_meter(&mut self, channel: usize, peak: f32) {
        if channel >= 2 {
            return;
        }
        let db = if peak < 1.0e-8 { -80.0 } else { 20.0 * peak.log10() };
        self.meter[channel] = if db > self.meter[channel] {
            db
        } else {
            self.meter[channel] * 0.9995 + db * 0.0005
        };
    }

    fn prepare_kernel(&mut self) {
        let channels = self.channels.len().max(1);
        if self.convolver.channels() != channels {
            self.convolver = BlockConvolver::new(channels);
        }
        self.convolver.set_kernel(&self.fir);
    }

    fn apply_coeffs(&mut self) {
        let sr = self.sample_rate;
        self.phase_mode = (self.values[PHASE_INDEX].round() as i32).clamp(0, 2) as u8;
        self.trim_lin = 10.0_f32.powf(self.values[TRIM_INDEX] / 20.0);
        for channel in &mut self.channels {
            configure_channel(channel, &self.values, sr);
        }
        let mut prototype = Channel::new();
        configure_channel(&mut prototype, &self.values, sr);
        self.fir = match self.phase_mode {
            1 => design_linear(&prototype),
            2 => design_analog_zl(&self.values, sr),
            _ => identity_fir(),
        };
        self.prepare_kernel();
    }
}

fn configure_channel(channel: &mut Channel, values: &[f32; PARAM_COUNT], sr: f32) {
    for band in 0..BANDS {
        let kind = filter_kind(values[type_index(band)]);
        let freq = values[freq_index(band)];
        let q = values[q_index(band)].max(0.2);
        let gain = if matches!(kind, FilterType::HighPass | FilterType::LowPass) {
            0.0
        } else {
            values[gain_index(band)]
        };
        channel.bands[band].configure(kind, sr, freq, q, gain);
    }
}

fn filter_kind(type_value: f32) -> FilterType {
    match type_value.round() as i32 {
        1 => FilterType::HighPass,
        2 => FilterType::LowPass,
        3 => FilterType::LowShelf,
        4 => FilterType::HighShelf,
        _ => FilterType::Peaking,
    }
}

fn host_stages(values: &[f32; PARAM_COUNT], rate: f32) -> Vec<Stage> {
    let mut stages = Vec::with_capacity(BANDS);
    for band in 0..BANDS {
        stages.push(stage_for_band(values, band, rate));
    }
    stages
}

fn stage_for_band(values: &[f32; PARAM_COUNT], band: usize, rate: f32) -> Stage {
    let kind = filter_kind(values[type_index(band)]);
    let gain = if matches!(kind, FilterType::HighPass | FilterType::LowPass) {
        0.0
    } else {
        values[gain_index(band)]
    };
    bilinear_stage(kind, rate, values[freq_index(band)], values[q_index(band)].max(0.2), gain)
}

fn design_linear(proto: &Channel) -> [f32; FIR_LEN] {
    let mut scratch = proto.clone();
    scratch.reset();
    let mut ir = [0.0f32; FIR_LEN];
    ir[0] = run_bands(&mut scratch, 1.0);
    for sample in ir.iter_mut().skip(1) {
        *sample = run_bands(&mut scratch, 0.0);
    }
    let mut re = ir;
    let mut im = [0.0f32; FIR_LEN];
    fft(&mut re, &mut im, false);
    for index in 0..FIR_LEN {
        let mag = (re[index] * re[index] + im[index] * im[index]).sqrt();
        re[index] = mag;
        im[index] = 0.0;
    }
    fft(&mut re, &mut im, true);
    let mut fir = [0.0f32; FIR_LEN];
    let mid = FIR_LEN / 2;
    for index in 0..FIR_LEN {
        fir[(index + mid) % FIR_LEN] = re[index];
    }
    fir
}

fn run_bands(channel: &mut Channel, x: f32) -> f32 {
    let mut y = x;
    for band in &mut channel.bands {
        y = band.process_sample(y);
    }
    y
}

fn design_analog_zl(values: &[f32; PARAM_COUNT], host_rate: f32) -> [f32; FIR_LEN] {
    let design_rate = host_rate * 16.0;
    let stages: Vec<Stage> = (0..BANDS).map(|band| stage_for_band(values, band, design_rate)).collect();
    let mut re = [0.0f32; FIR_LEN];
    let mut im = [0.0f32; FIR_LEN];
    for bin in 0..=FIR_LEN / 2 {
        let freq = if bin == 0 { 1.0 } else { bin as f32 * host_rate / FIR_LEN as f32 };
        let mut hr = 1.0f32;
        let mut hi = 0.0f32;
        for stage in &stages {
            let (sr, si) = stage_response(stage, freq, design_rate);
            let nr = hr * sr - hi * si;
            hi = hr * si + hi * sr;
            hr = nr;
        }
        re[bin] = hr;
        im[bin] = hi;
        if bin > 0 && bin < FIR_LEN / 2 {
            re[FIR_LEN - bin] = hr;
            im[FIR_LEN - bin] = -hi;
        }
    }
    fft(&mut re, &mut im, true);
    let mut fir = [0.0f32; FIR_LEN];
    let keep = FIR_LEN / 2;
    for i in 0..keep {
        let fade = if i > keep - 64 { (keep - 1 - i) as f32 / 64.0 } else { 1.0 };
        fir[i] = re[i] * fade;
    }
    fir
}

struct Stage {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

fn bilinear_stage(filter: FilterType, design_rate: f32, freq: f32, q: f32, gain_db: f32) -> Stage {
    let sr = design_rate.max(1000.0);
    let f0 = freq.clamp(10.0, sr * 0.499);
    let q = q.max(0.05);
    let w0 = std::f32::consts::TAU * f0 / sr;
    let cos_w0 = w0.cos();
    let alpha = w0.sin() / (2.0 * q);
    let a = 10.0_f32.powf(gain_db / 40.0);
    let (b0, b1, b2, a0, a1, a2) = match filter {
        FilterType::LowPass => {
            let b1 = 1.0 - cos_w0;
            let b0 = b1 * 0.5;
            (b0, b1, b0, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
        }
        FilterType::HighPass => {
            let b0 = (1.0 + cos_w0) * 0.5;
            (b0, -(1.0 + cos_w0), b0, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
        }
        FilterType::LowShelf => {
            let a_plus = a + 1.0;
            let a_minus = a - 1.0;
            let s = 2.0 * a.sqrt() * alpha;
            (
                a * (a_plus - a_minus * cos_w0 + s),
                2.0 * a * (a_minus - a_plus * cos_w0),
                a * (a_plus - a_minus * cos_w0 - s),
                a_plus + a_minus * cos_w0 + s,
                -2.0 * (a_minus + a_plus * cos_w0),
                a_plus + a_minus * cos_w0 - s,
            )
        }
        FilterType::HighShelf => {
            let a_plus = a + 1.0;
            let a_minus = a - 1.0;
            let s = 2.0 * a.sqrt() * alpha;
            (
                a * (a_plus + a_minus * cos_w0 + s),
                -2.0 * a * (a_minus + a_plus * cos_w0),
                a * (a_plus + a_minus * cos_w0 - s),
                a_plus - a_minus * cos_w0 + s,
                2.0 * (a_minus - a_plus * cos_w0),
                a_plus - a_minus * cos_w0 - s,
            )
        }
        _ => (1.0 + alpha * a, -2.0 * cos_w0, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos_w0, 1.0 - alpha / a),
    };
    let inv = 1.0 / a0;
    Stage { b0: b0 * inv, b1: b1 * inv, b2: b2 * inv, a1: a1 * inv, a2: a2 * inv }
}

fn stage_response(stage: &Stage, freq: f32, rate: f32) -> (f32, f32) {
    let w = std::f32::consts::TAU * freq / rate.max(1.0);
    let z1_re = w.cos();
    let z1_im = -w.sin();
    let z2_re = z1_re * z1_re - z1_im * z1_im;
    let z2_im = 2.0 * z1_re * z1_im;
    let num_re = stage.b0 + stage.b1 * z1_re + stage.b2 * z2_re;
    let num_im = stage.b1 * z1_im + stage.b2 * z2_im;
    let den_re = 1.0 + stage.a1 * z1_re + stage.a2 * z2_re;
    let den_im = stage.a1 * z1_im + stage.a2 * z2_im;
    let den = den_re * den_re + den_im * den_im;
    if den < 1.0e-20 {
        return (1.0, 0.0);
    }
    ((num_re * den_re + num_im * den_im) / den, (num_im * den_re - num_re * den_im) / den)
}

fn identity_fir() -> [f32; FIR_LEN] {
    let mut fir = [0.0; FIR_LEN];
    fir[FIR_LEN / 2] = 1.0;
    fir
}

fn fft(re: &mut [f32], im: &mut [f32], inverse: bool) {
    let n = re.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = std::f32::consts::TAU / len as f32 * if inverse { 1.0 } else { -1.0 };
        let wlen_re = ang.cos();
        let wlen_im = ang.sin();
        let mut i = 0;
        while i < n {
            let mut w_re = 1.0f32;
            let mut w_im = 0.0f32;
            for k in 0..len / 2 {
                let u_re = re[i + k];
                let u_im = im[i + k];
                let v_re = re[i + k + len / 2] * w_re - im[i + k + len / 2] * w_im;
                let v_im = re[i + k + len / 2] * w_im + im[i + k + len / 2] * w_re;
                re[i + k] = u_re + v_re;
                im[i + k] = u_im + v_im;
                re[i + k + len / 2] = u_re - v_re;
                im[i + k + len / 2] = u_im - v_im;
                let next_re = w_re * wlen_re - w_im * wlen_im;
                w_im = w_re * wlen_im + w_im * wlen_re;
                w_re = next_re;
            }
            i += len;
        }
        len <<= 1;
    }
    if inverse {
        let scale = 1.0 / n as f32;
        for value in re.iter_mut() {
            *value *= scale;
        }
        for value in im.iter_mut() {
            *value *= scale;
        }
    }
}

fn defaults() -> [f32; PARAM_COUNT] {
    let mut values = [0.0; PARAM_COUNT];
    for (index, param) in crate::params::PARAMS.iter().enumerate() {
        values[index] = param.default;
    }
    values
}

impl Default for EqDsp {
    fn default() -> Self {
        Self::new()
    }
}
