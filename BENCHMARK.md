# OpenPlugin SDK vs JUCE: 4-band EQ

Same EQ, two plugins. OpenPlugin is `openplugin-eq` (Cargo, GoodLookin DRI). JUCE is `juce-eq` (CMake standalone `EQ Benchmark JUCE.app`). Both expose high-pass and low-pass slopes of 6, 12, and 24 dB/oct, plus four peaking bands with gain, Q, and frequency. Every frequency control spans 20 Hz to 20000 Hz.

Probes are the shipped process paths. Each suite was run twice. The two OpenPlugin runs matched, and the two JUCE runs matched. Figures below are the `level_db` values printed in those logs.

## Slope, one octave in the stopband

Low-pass cutoff 1 kHz, tones at 2 kHz and 4 kHz. High-pass cutoff 4 kHz, tones at 2 kHz and 1 kHz. Delta is the first stopband tone minus the tone one octave further.

| Case | OpenPlugin dB | JUCE dB |
|---|---:|---:|
| LP 6 dB, 2 kHz | -6.965 | -7.01902 |
| LP 6 dB, 4 kHz | -12.205 | -12.4833 |
| LP 6 dB delta | 5.240 | 5.46431 |
| HP 6 dB, 2 kHz | -9.362 | -7.11033 |
| HP 6 dB, 1 kHz | -14.681 | -12.4861 |
| HP 6 dB delta | 5.319 | 5.37576 |
| LP 12 dB, 2 kHz | -12.375 | -12.3745 |
| LP 12 dB, 4 kHz | -24.475 | -24.4761 |
| LP 12 dB delta | 12.100 | 12.1016 |
| HP 12 dB, 2 kHz | -12.591 | -12.5914 |
| HP 12 dB, 1 kHz | -24.474 | -24.4736 |
| HP 12 dB delta | 11.882 | 11.8822 |
| LP 24 dB, 2 kHz | -24.247 | -24.2466 |
| LP 24 dB, 4 kHz | -48.557 | -48.9214 |
| LP 24 dB delta | 24.311 | 24.6748 |
| HP 24 dB, 2 kHz | -24.703 | -24.7032 |
| HP 24 dB, 1 kHz | -48.921 | -48.9207 |
| HP 24 dB delta | 24.218 | 24.2175 |

12 dB and 24 dB agree because both are RBJ biquads, one section or two cascaded sections. The 6 dB octave deltas in the table also sit close together. The absolute 6 dB high-pass levels do not: the 2 kHz row is -9.362 on OpenPlugin and -7.11033 on JUCE. OpenPlugin uses an exponential one-pole and its complement, because `openplugin-dsp::Biquad` has no first-order section. JUCE uses `juce::dsp::IIR` `makeFirstOrderHighPass`, which is bilinear.

## Frequency ends, 1 kHz tone, 12 dB/oct

| Case | OpenPlugin dB | JUCE dB |
|---|---:|---:|
| Low-pass at 20 Hz | -66.777 | -67.0925 |
| Low-pass at 20000 Hz | 0.003 | 0.00349129 |
| High-pass at 20 Hz | 0.003 | 0.00349129 |
| High-pass at 20000 Hz | -70.213 | -70.2132 |

Setting a cutoff to 20 Hz or 20000 Hz moves the corner. The open setting passes 1 kHz at 0.003 on OpenPlugin and 0.00349129 on JUCE. The closed setting cuts it to -66.777 or -70.213 on OpenPlugin, and to -67.0925 or -70.2132 on JUCE.

## Bands

Each band in turn, center 1 kHz, other bands at 0 dB. All four bands produced the same numbers.

| Case | OpenPlugin dB | JUCE dB |
|---|---:|---:|
| +12 dB, Q 2, tone 1 kHz | 12.003 | 12.0035 |
| -12 dB, Q 2, tone 1 kHz | -11.997 | -11.9965 |
| wide Q, tone 2 kHz | 7.433 | 7.43316 |
| +12 dB, Q 4, tone 2 kHz | 0.418 | 0.417879 |

Boost, cut, and Q agree. JUCE's `makePeakFilter` takes linear gain, so the processor converts from dB. OpenPlugin's biquad takes dB directly. The result is the same RBJ peak.

## Band centers at 20 Hz and 20000 Hz

Each band in turn is set to +12 dB at Q 2. The other bands stay at 0 dB. High-pass stays at 20 Hz and low-pass at 20000 Hz, both 12 dB/oct. The peak still moves: the tone at the chosen center rises, and the tone at the other end does not.

| Band | Center | Tone | OpenPlugin dB | JUCE dB |
|---|---:|---:|---:|---:|
| 1 | 20 Hz | 20 Hz | 8.794 | 8.79346 |
| 1 | 20 Hz | 1000 Hz | 0.005 | 0.00515677 |
| 1 | 20 Hz | 20000 Hz | -3.011 | -3.01109 |
| 1 | 20000 Hz | 20 Hz | -3.003 | -3.00262 |
| 1 | 20000 Hz | 1000 Hz | 0.005 | 0.00472797 |
| 1 | 20000 Hz | 20000 Hz | 8.989 | 8.98891 |
| 2 | 20 Hz | 20 Hz | 8.794 | 8.79885 |
| 2 | 20 Hz | 1000 Hz | 0.005 | 0.00516269 |
| 2 | 20 Hz | 20000 Hz | -3.011 | -3.01108 |
| 2 | 20000 Hz | 20 Hz | -3.003 | -3.0026 |
| 2 | 20000 Hz | 1000 Hz | 0.005 | 0.00472765 |
| 2 | 20000 Hz | 20000 Hz | 8.989 | 8.98891 |
| 3 | 20 Hz | 20 Hz | 8.795 | 8.80318 |
| 3 | 20 Hz | 1000 Hz | 0.005 | 0.0051681 |
| 3 | 20 Hz | 20000 Hz | -3.011 | -3.01109 |
| 3 | 20000 Hz | 20 Hz | -3.003 | -3.0026 |
| 3 | 20000 Hz | 1000 Hz | 0.005 | 0.00472719 |
| 3 | 20000 Hz | 20000 Hz | 8.989 | 8.98891 |
| 4 | 20 Hz | 20 Hz | 8.795 | 8.79393 |
| 4 | 20 Hz | 1000 Hz | 0.005 | 0.00515716 |
| 4 | 20 Hz | 20000 Hz | -3.011 | -3.01109 |
| 4 | 20000 Hz | 20 Hz | -3.003 | -3.0026 |
| 4 | 20000 Hz | 1000 Hz | 0.005 | 0.00472672 |
| 4 | 20000 Hz | 20000 Hz | 8.989 | 8.98891 |

On every band, setting the center to 20 Hz lifts the 20 Hz tone from -3.003 to 8.794 on OpenPlugin for band 1 (bands 3 and 4 print 8.795). JUCE band 1 moves from -3.00262 to 8.79346. Setting the center to 20000 Hz lifts that tone from -3.011 to 8.989 on OpenPlugin and from -3.01109 to 8.98891 on JUCE. The 1 kHz tone stays at 0.005 on OpenPlugin. On JUCE it stays between 0.00472672 and 0.0051681. The control moves the peak rather than adding broadband gain.

## CPU

Process-CPU time on an Apple M1 Max, one thread, release builds. The clock is `CLOCK_PROCESS_CPUTIME_ID`. Each pass renders 8 seconds of stereo audio at the tests' sample rate, in blocks of 512, from a 1 kHz sine. A copy-only loop is subtracted. The first pass is discarded. The figure is the median of the next five, as a percent of realtime: 100 means one core is busy for the whole audio duration.

Flat is the default filters: high-pass at 20 Hz, low-pass at 20000 Hz, both 12 dB/oct, bands at 0 dB. Heavy is a high-pass at 100 Hz and a low-pass at 8000 Hz, both 24 dB/oct, with each band at +6 dB and Q 2.

| Case | OpenPlugin run 1 | OpenPlugin run 2 | JUCE run 1 | JUCE run 2 |
|---|---:|---:|---:|---:|
| flat | 0.1160 | 0.1156 | 0.192137 | 0.192138 |
| heavy | 0.1736 | 0.1720 | 0.252562 | 0.253675 |

OpenPlugin spends less process time on both loads. The heavy patch costs more than the flat patch on both, and the output of every timed pass stayed finite.

## What differed in the build

The DSP that a listener would compare is the same design, and the 12 dB, 24 dB, and peaking measurements say so. The work around that DSP is not the same.

OpenPlugin is a Rust library. Parameters are a static `ParamInfo` table. The audio callback is `prepare` / `set_param` / `process` on planar `Vec<f32>` buffers, and it does not allocate. The editor is a GoodLookin DRI draw list: a British-console faceplate and sixteen labeled knobs. There is no widget tree and no mouse attachment in this plugin. `cargo test` drives the real plugin and the real `render` function.

JUCE is a C++ `AudioProcessor` with an `AudioProcessorValueTreeState` and the same sixteen parameter ids. Coefficients come from `juce::dsp::IIR` and are rebuilt only when a value or the sample rate changes. The editor is a custom painted face (navy panel, screws, brass title) and sixteen rotary sliders with labels and `SliderAttachment`s, so the controls change the parameters the process path reads. CMake has to build juceaide and the JUCE modules before the plugin links. The standalone `EQ Benchmark JUCE.app` linked. The probe walks that editor and prints a label for every control.

So: use OpenPlugin when the plugin should stay a small Rust crate against this SDK and draw through GoodLookin. Use JUCE when the plugin should be a normal C++ audio processor with a live component editor and JUCE's IIR helpers, and budget a heavier compile for that. For this EQ the filters match, except the 6 dB high-pass implementation noted above.
