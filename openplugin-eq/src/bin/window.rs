//! Standalone window for the OpenPlugin EQ.
//!
//! The picture is a GoodLookin scene. The shell opens the Cocoa window and sends the mouse.
//! The spectrum is `goodlookindri::spectrum`: the audio callback only pushes `SampleRing`,
//! and the UI thread runs the FFT. A click on empty plot freezes the spectrum, the next
//! click turns it off, and the one after that brings it back.
//!
//! `openplugin-eq --smoke` draws for a moment and exits 0 when the frame is not blank.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, Stream};
use goodlookindri_shell::{render_offscreen, run, App, Config, Event};
use openplugin::dri::goodlookindri::scene::Scene;
use openplugin::dri::goodlookindri::spectrum::SampleRing;
use openplugin::prelude::Plugin;
use openplugin_eq::{paint, pointer_down, pointer_drag, spectrum_view, EqPlugin, Gesture, Spectrum};

const WIDTH: f32 = 980.0;
const HEIGHT: f32 = 640.0;

struct Engine {
    plugin: EqPlugin,
    left: Vec<f32>,
    right: Vec<f32>,
    phase: f32,
}

impl Engine {
    fn new(sample_rate: f32) -> Self {
        let mut plugin = EqPlugin::new();
        plugin.prepare(sample_rate, 512, 2);
        Self {
            plugin,
            left: vec![0.0; 512],
            right: vec![0.0; 512],
            phase: 0.0,
        }
    }

    fn render_block(&mut self, frames: usize, ring: &SampleRing) {
        if self.left.len() < frames {
            self.left.resize(frames, 0.0);
            self.right.resize(frames, 0.0);
        }
        let rate = self.plugin.sample_rate().max(1000.0);
        for index in 0..frames {
            self.phase = (self.phase + 1.0) % rate;
            let t = self.phase;
            let noise = ((t as u32).wrapping_mul(1_664_525).wrapping_add(1_013_904_223) >> 16) as f32 / 32768.0 - 1.0;
            let x = (t * 0.13).sin() * 0.18 + (t * 0.051).sin() * 0.10 + (t * 0.79).sin() * 0.05 + noise * 0.04;
            self.left[index] = x;
            self.right[index] = x;
        }
        self.plugin.process_stereo(&mut self.left[..frames], &mut self.right[..frames]);
        ring.push(&[&self.left[..frames], &self.right[..frames]]);
    }
}

struct Face {
    engine: Arc<Mutex<Engine>>,
    ring: Arc<SampleRing>,
    spectrum: Spectrum,
    gesture: Gesture,
    dragging: bool,
    size: (f32, f32),
    sample_rate: f64,
    live: bool,
    _stream: Option<Stream>,
}

impl Face {
    fn pump(&mut self, dt: f32) {
        let frames = ((self.sample_rate * dt as f64) as usize).clamp(64, 2048);
        let mut engine = self.engine.lock().unwrap_or_else(|error| error.into_inner());
        engine.render_block(frames, &self.ring);
    }
}

impl App for Face {
    fn event(&mut self, event: Event) -> bool {
        match event {
            Event::Tick { dt } => {
                if !self.live {
                    self.pump(dt);
                }
                self.spectrum.update(&self.ring, dt, self.sample_rate);
                true
            }
            Event::MouseDown { x, y } => {
                let (width, height) = self.size;
                let mut engine = self.engine.lock().unwrap_or_else(|error| error.into_inner());
                self.dragging = pointer_down(&mut engine.plugin, width, height, x, y, &mut self.gesture);
                // Empty plot: live, then frozen, then off.
                if self.gesture.on_plot() {
                    self.spectrum.next_mode();
                }
                true
            }
            Event::MouseMove { x, y } => {
                if !self.dragging {
                    return false;
                }
                let (width, height) = self.size;
                let mut engine = self.engine.lock().unwrap_or_else(|error| error.into_inner());
                pointer_drag(&mut engine.plugin, width, height, x, y, &self.gesture);
                true
            }
            Event::MouseUp { .. } => {
                self.dragging = false;
                true
            }
            Event::MouseWheel { .. } | Event::Key { .. } => false,
        }
    }

    fn paint(&mut self, scene: &mut Scene, width: f32, height: f32) {
        self.size = (width, height);
        let engine = self.engine.lock().unwrap_or_else(|error| error.into_inner());
        paint(&engine.plugin, scene, width, height, Some(&mut self.spectrum));
    }
}

fn open_output(engine: Arc<Mutex<Engine>>, ring: Arc<SampleRing>) -> Option<(Stream, f64)> {
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let range = device.supported_output_configs().ok()?.find(|config| {
        config.channels() >= 2
            && config.sample_format() == SampleFormat::F32
            && config.min_sample_rate() <= SampleRate(48000)
            && config.max_sample_rate() >= SampleRate(48000)
    })?;
    let config: cpal::StreamConfig = range.with_sample_rate(SampleRate(48000)).into();
    let channels = config.channels as usize;
    {
        let mut engine = engine.lock().unwrap_or_else(|error| error.into_inner());
        engine.plugin.prepare(48000.0, 512, channels.max(2));
    }
    let engine_for_audio = engine.clone();
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                let frames = data.len() / channels.max(1);
                if frames == 0 {
                    return;
                }
                let mut engine = engine_for_audio.lock().unwrap_or_else(|error| error.into_inner());
                engine.render_block(frames, &ring);
                for index in 0..frames {
                    if channels == 1 {
                        data[index] = engine.left[index];
                    } else {
                        data[index * channels] = engine.left[index];
                        data[index * channels + 1] = engine.right[index];
                    }
                }
            },
            |error| eprintln!("audio {error}"),
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some((stream, 48000.0))
}

fn face(sample_rate: f64, live: bool) -> Face {
    let ring = Arc::new(SampleRing::new());
    let engine = Arc::new(Mutex::new(Engine::new(sample_rate as f32)));
    Face {
        engine,
        ring,
        spectrum: spectrum_view(),
        gesture: Gesture::default(),
        dragging: false,
        size: (WIDTH, HEIGHT),
        sample_rate,
        live,
        _stream: None,
    }
}

fn render_ppm(path: &str, width: u32, height: u32) -> i32 {
    let mut app = face(48000.0, false);
    for _ in 0..45 {
        app.event(Event::Tick { dt: 1.0 / 30.0 });
    }
    let frame = render_offscreen(&mut app, width, height, 1.0);
    match std::fs::write(path, frame.to_ppm()) {
        Ok(()) => {
            println!("wrote {path} ({}x{})", frame.width, frame.height);
            0
        }
        Err(error) => {
            eprintln!("{path}: {error}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--render") {
        let path = args.get(1).map(String::as_str).unwrap_or("openplugin-eq.ppm");
        let width = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(WIDTH as u32);
        let height = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(HEIGHT as u32);
        std::process::exit(render_ppm(path, width, height));
    }
    let smoke = args.iter().any(|arg| arg == "--smoke");
    let ring = Arc::new(SampleRing::new());
    let engine = Arc::new(Mutex::new(Engine::new(48000.0)));
    let opened = open_output(engine.clone(), ring.clone());
    let (stream, sample_rate, live) = match opened {
        Some((stream, rate)) => (Some(stream), rate, true),
        None => {
            eprintln!("no 48 kHz output; drawing the FFT from a local tone");
            (None, 48000.0, false)
        }
    };
    let face = Face {
        engine,
        ring,
        spectrum: spectrum_view(),
        gesture: Gesture::default(),
        dragging: false,
        size: (WIDTH, HEIGHT),
        sample_rate,
        live,
        _stream: stream,
    };
    let code = run(
        Config {
            title: "FreEQ".into(),
            width: WIDTH as u32,
            height: HEIGHT as u32,
            smoke_ms: if smoke { Some(800) } else { None },
        },
        Box::new(face),
    );
    std::process::exit(code);
}
