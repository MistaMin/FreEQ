//! Four-band EQ on OpenPlugin SDK. DSP is `dsp`, the plugin trait is `plugin`, the
//! GoodLookin DRI editor is `editor`.

pub mod dsp;
pub mod editor;
pub mod params;
pub mod plugin;

pub use editor::{paint, pointer_down, pointer_drag, render, spectrum_view, Gesture};
pub use plugin::EqPlugin;
