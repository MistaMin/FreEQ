//! Six bands. Each band is a peak, a high pass, a low pass, a low shelf, or a high shelf.

use openplugin::prelude::*;

pub const BANDS: usize = 6;
pub const PARAM_COUNT: usize = BANDS * 4 + 2;
pub const PHASE_CHOICES: &[&str] = &["Minimum", "Linear", "Analog ZL"];
pub const TYPE_CHOICES: &[&str] = &["Peak", "High Pass", "Low Pass", "Low Shelf", "High Shelf"];

pub const fn type_index(band: usize) -> usize {
    band * 4
}
pub const fn freq_index(band: usize) -> usize {
    band * 4 + 1
}
pub const fn gain_index(band: usize) -> usize {
    band * 4 + 2
}
pub const fn q_index(band: usize) -> usize {
    band * 4 + 3
}
pub const PHASE_INDEX: usize = BANDS * 4;
pub const TRIM_INDEX: usize = BANDS * 4 + 1;

const DEFAULT_FREQ: [f32; BANDS] = [50.0, 92.0, 500.0, 3339.0, 7885.0, 10992.0];
const DEFAULT_GAIN: [f32; BANDS] = [0.0, 0.0, 7.9, 2.8, 0.0, 0.0];
const DEFAULT_Q: [f32; BANDS] = [0.90, 0.98, 0.48, 0.80, 0.80, 0.90];
const DEFAULT_TYPE: [f32; BANDS] = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];

pub static PARAMS: [ParamInfo; PARAM_COUNT] = [
    ParamInfo::new("b1_type", "Band 1 Type", 0.0, 4.0, DEFAULT_TYPE[0], Unit::Choice).with_choices(TYPE_CHOICES),
    ParamInfo::new("b1_freq", "Band 1 Freq", 20.0, 20000.0, DEFAULT_FREQ[0], Unit::Hz).with_taper(Taper::Log),
    ParamInfo::new("b1_gain", "Band 1 Gain", -18.0, 18.0, DEFAULT_GAIN[0], Unit::Db),
    ParamInfo::new("b1_q", "Band 1 Q", 0.2, 8.0, DEFAULT_Q[0], Unit::None).with_taper(Taper::Log),
    ParamInfo::new("b2_type", "Band 2 Type", 0.0, 4.0, DEFAULT_TYPE[1], Unit::Choice).with_choices(TYPE_CHOICES),
    ParamInfo::new("b2_freq", "Band 2 Freq", 20.0, 20000.0, DEFAULT_FREQ[1], Unit::Hz).with_taper(Taper::Log),
    ParamInfo::new("b2_gain", "Band 2 Gain", -18.0, 18.0, DEFAULT_GAIN[1], Unit::Db),
    ParamInfo::new("b2_q", "Band 2 Q", 0.2, 8.0, DEFAULT_Q[1], Unit::None).with_taper(Taper::Log),
    ParamInfo::new("b3_type", "Band 3 Type", 0.0, 4.0, DEFAULT_TYPE[2], Unit::Choice).with_choices(TYPE_CHOICES),
    ParamInfo::new("b3_freq", "Band 3 Freq", 20.0, 20000.0, DEFAULT_FREQ[2], Unit::Hz).with_taper(Taper::Log),
    ParamInfo::new("b3_gain", "Band 3 Gain", -18.0, 18.0, DEFAULT_GAIN[2], Unit::Db),
    ParamInfo::new("b3_q", "Band 3 Q", 0.2, 8.0, DEFAULT_Q[2], Unit::None).with_taper(Taper::Log),
    ParamInfo::new("b4_type", "Band 4 Type", 0.0, 4.0, DEFAULT_TYPE[3], Unit::Choice).with_choices(TYPE_CHOICES),
    ParamInfo::new("b4_freq", "Band 4 Freq", 20.0, 20000.0, DEFAULT_FREQ[3], Unit::Hz).with_taper(Taper::Log),
    ParamInfo::new("b4_gain", "Band 4 Gain", -18.0, 18.0, DEFAULT_GAIN[3], Unit::Db),
    ParamInfo::new("b4_q", "Band 4 Q", 0.2, 8.0, DEFAULT_Q[3], Unit::None).with_taper(Taper::Log),
    ParamInfo::new("b5_type", "Band 5 Type", 0.0, 4.0, DEFAULT_TYPE[4], Unit::Choice).with_choices(TYPE_CHOICES),
    ParamInfo::new("b5_freq", "Band 5 Freq", 20.0, 20000.0, DEFAULT_FREQ[4], Unit::Hz).with_taper(Taper::Log),
    ParamInfo::new("b5_gain", "Band 5 Gain", -18.0, 18.0, DEFAULT_GAIN[4], Unit::Db),
    ParamInfo::new("b5_q", "Band 5 Q", 0.2, 8.0, DEFAULT_Q[4], Unit::None).with_taper(Taper::Log),
    ParamInfo::new("b6_type", "Band 6 Type", 0.0, 4.0, DEFAULT_TYPE[5], Unit::Choice).with_choices(TYPE_CHOICES),
    ParamInfo::new("b6_freq", "Band 6 Freq", 20.0, 20000.0, DEFAULT_FREQ[5], Unit::Hz).with_taper(Taper::Log),
    ParamInfo::new("b6_gain", "Band 6 Gain", -18.0, 18.0, DEFAULT_GAIN[5], Unit::Db),
    ParamInfo::new("b6_q", "Band 6 Q", 0.2, 8.0, DEFAULT_Q[5], Unit::None).with_taper(Taper::Log),
    ParamInfo::new("phase", "Phase", 0.0, 2.0, 0.0, Unit::Choice).with_choices(PHASE_CHOICES),
    ParamInfo::new("trim", "Trim", -24.0, 24.0, 0.0, Unit::Db),
];

pub const BAND_COLOR: [(f32, f32, f32); BANDS] = [
    (0.91, 0.55, 0.48),
    (0.93, 0.82, 0.28),
    (0.28, 0.66, 0.38),
    (0.28, 0.58, 0.82),
    (0.58, 0.36, 0.72),
    (0.82, 0.36, 0.58),
];
