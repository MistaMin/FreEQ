//! Four-band EQ on OpenPlugin SDK. DSP is `dsp`, the plugin trait is `plugin`, the
//! GoodLookin DRI editor is `editor`.

pub mod dsp;
pub mod editor;
pub mod params;
pub mod plugin;

pub use editor::{paint, pointer_down, pointer_drag, render, spectrum_view, Gesture, Spectrum, SpectrumMode};
pub use plugin::EqPlugin;

openplugin_targets::export_clap!(EqPlugin);
openplugin_targets::export_lv2!(EqPlugin);
openplugin_targets::export_shell!(EqPlugin);
