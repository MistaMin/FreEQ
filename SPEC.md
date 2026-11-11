# EQ Benchmark contract

Two plugins, one EQ. OpenPlugin lives in `openplugin-eq/`. JUCE lives in `juce-eq/`. Do not edit the OpenPluginSDK or JUCE trees. Do not modify this file.

## Parameters (same ids, ranges, defaults, order)

| Index | Id | Range | Default | Taper | Unit |
|---|---|---|---|---|---|
| 0 | hp_freq | 20 .. 20000 Hz | 20 | log | Hz |
| 1 | hp_slope | 0, 1, 2 → 6 dB, 12 dB, 24 dB | 1 (12 dB) | linear | choice |
| 2 | lp_freq | 20 .. 20000 Hz | 20000 | log | Hz |
| 3 | lp_slope | 0, 1, 2 → 6 dB, 12 dB, 24 dB | 1 (12 dB) | linear | choice |
| 4 | b1_freq | 20 .. 20000 | 100 | log | Hz |
| 5 | b1_gain | -18 .. 18 dB | 0 | linear | dB |
| 6 | b1_q | 0.3 .. 8 | 0.707 | log | — |
| 7 | b2_freq | 20 .. 20000 | 400 | log | Hz |
| 8 | b2_gain | -18 .. 18 | 0 | linear | dB |
| 9 | b2_q | 0.3 .. 8 | 0.707 | log | — |
| 10 | b3_freq | 20 .. 20000 | 1600 | log | Hz |
| 11 | b3_gain | -18 .. 18 | 0 | linear | dB |
| 12 | b3_q | 0.3 .. 8 | 0.707 | log | — |
| 13 | b4_freq | 20 .. 20000 | 6400 | log | Hz |
| 14 | b4_gain | -18 .. 18 | 0 | linear | dB |
| 15 | b4_q | 0.3 .. 8 | 0.707 | log | — |

Slope index 0 is 6 dB/oct, 1 is 12 dB/oct, 2 is 24 dB/oct. Clamp every value before it reaches the filters. NaN falls back to the default.

## Signal path

Per sample, per channel, in this order:

1. High-pass at `hp_freq` with the selected slope.
2. Low-pass at `lp_freq` with the selected slope.
3. Four RBJ peaking bands, band 1 then 2 then 3 then 4.

A peaking band at 0 dB is exactly unity. Q is the RBJ Q.

Slope filters, Butterworth-aligned:

- 6 dB/oct: one-pole. `g = 1 - exp(-2π fc / sr)`. Low-pass: `y += g * (x - y)`. High-pass: the complement of its own one-pole low-pass (`x - y`), with its own state.
- 12 dB/oct: one RBJ biquad low-pass or high-pass, `Q = 1/sqrt(2)`.
- 24 dB/oct: two cascaded RBJ biquads, `Q = 0.54119610` then `Q = 1.30656296`.

OpenPlugin must use `openplugin_dsp::Biquad` for the biquads and the peaking bands. Coefficient updates happen in `prepare` and `set_params`, never inside the per-sample loop. `process` / `process_sample` must not allocate. Support at least 2 channels. Reset clears filter memory.

JUCE must use `juce::dsp::IIR` (first-order for 6 dB, RBJ low-pass / high-pass / peak for the rest) with the same Q values. Rebuild coefficients only when a parameter or the sample rate changes.

## Editor labels

Every label below is a visible label on a real control. OpenPlugin passes these strings to `draw_knob` (they land in the draw list as text). JUCE sets them on a `juce::Label` and sets the same string as the slider's component name.

- HP Freq
- HP Slope
- LP Freq
- LP Slope
- Band 1 Freq, Band 1 Gain, Band 1 Q
- Band 2 Freq, Band 2 Gain, Band 2 Q
- Band 3 Freq, Band 3 Gain, Band 3 Q
- Band 4 Freq, Band 4 Gain, Band 4 Q

OpenPlugin also draws one GoodLookin faceplate (`draw_faceplate` or `draw_faceplate_code`) behind the knobs. Editor size is 980×460.

## Probes (both DSP tests print these lines)

Sample rate 48000. Drive a sine of peak 0.25. Discard the first 8192 output samples. RMS the next 8192. Report `20*log10(rms_out/rms_in)`.

Print exactly:

```
PROBE case=<name> slope=<6|12|24> tone_hz=<hz> level_db=<number>
```

Cases:

- `lp_slope`: hp_freq=20, lp_freq=1000, all band gains 0, Q 0.707. Tones 2000 Hz and 4000 Hz. Repeat for slopes 6, 12, 24.
- `hp_slope`: hp_freq=4000, lp_freq=20000, all band gains 0. Tones 2000 Hz and 1000 Hz. Repeat for slopes 6, 12, 24.
- `lp_at_20` / `lp_at_20000`: slope 12, tone 1000 Hz, lp_freq 20 then 20000, hp_freq 20, bands flat.
- `hp_at_20` / `hp_at_20000`: slope 12, tone 1000 Hz, hp_freq 20 then 20000, lp_freq 20000, bands flat.
- `band_boost` / `band_cut`: for each band index 1..4, set that band freq=1000, Q=2, gain=+12 then -12, other gains 0, hp_freq=20, lp_freq=20000, slopes 12. Tone 1000 Hz.
- `band_q`: for each band, gain=+12, freq=1000, tone 2000 Hz, once at Q=0.5 and once at Q=4.

Expected, measured on the real process path:

- Octave delta (`level` at the first stopband tone minus `level` at the tone one octave further) within 3 dB of 6, 12, or 24.
- `lp_at_20` and `hp_at_20000` below -24 dB. `lp_at_20000` and `hp_at_20` within 1.5 dB of 0.
- Boost within 2 dB of +12. Cut within 2 dB of -12.
- Q=0.5 boosts the 2000 Hz tone more than Q=4, by at least 2 dB.
