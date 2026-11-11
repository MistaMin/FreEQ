# JUCE update and EQ benchmark

The EQ benchmark now builds against the current JUCE 8 security release and against official JUCE 9.0.5. The two AudioGridder JUCE 9 trees were moved onto 9.0.5 with their local patches kept.

## Versions

| Tree | Path | Was | Now | Commit |
| --- | --- | --- | --- | --- |
| JUCE 8 used by this EQ | `/Volumes/Developer 2/Dependencies/JUCE-8.0.4` | 8.0.4 (`51d11a2`, 18 Nov 2024) | 8.0.17-security | `dabd5ad13c`, 2 Nov 2026 |
| Official JUCE 9 used by this EQ | `/Volumes/Developer 2/Dependencies/JUCE-9.0.5` | not present | 9.0.5 | `d60b7a7803`, 2 Nov 2026 |
| AudioGridder JUCE | `/Volumes/Developer/OpenGrid/audiogridder/JUCE` | 9.0.4 plus three local commits (`a480a36d4e`) | 9.0.5 plus the same three patches | `22c3b22937` |
| AudioGridder JUCE, second checkout | `/Volumes/Developer/OpenGrid/audiogridder-from-Projects/JUCE` | same as above | same patch, rebased separately | `100f412b6d` |

The directory `JUCE-8.0.4` kept its name so the existing CMake path still resolves. Its contents are tag `8.0.17-security`. The version macros are `JUCE_MAJOR_VERSION 8`, `JUCE_BUILDNUMBER 17`.

`JUCE-9.0.5` is a git worktree of that same clone, checked out at tag `9.0.5`. The EQ's JUCE 9 build uses this official tree, not the AudioGridder fork.

Published current releases on 2 November 2026 were 9.0.5 and 8.0.17-security. 8.0.4 was not current.

## AudioGridder patches

Both JUCE 9 forks were detached and clean. They were not plain 9.0.4. Three local commits sat on top:

1. `Port AudioGridder patches onto JUCE 9.0.4` — socket and interprocess changes, about 500 lines.
2. `Update AudioPluginHost launchWorkerProcess call to match fork signature`.
3. `AU: only use the no-input shortcut for MIDI effects without an audio input bus`.

`git rebase --onto 9.0.5 9.0.4` applied all three with no conflicts. The commit messages were left as they were, so the first still says "9.0.4" even though its parent is `d60b7a7803` (9.0.5). The two working trees have different commit hashes because they were rebased separately. `git diff 9.0.5` is identical in both (918 lines).

These forks were not pushed.

## EQ builds

`juce-eq/CMakeLists.txt` takes `-DEQ_JUCE_PATH`. The default is the JUCE 8 tree.

| Build | Configure | Probe |
| --- | --- | --- |
| JUCE 8.0.17-security | `juce-eq/build` | `juce-eq/build/EqProbe_artefacts/Release/EqProbe` |
| JUCE 9.0.5 | `juce-eq/build-9`, `-DEQ_JUCE_PATH=.../JUCE-9.0.5 -DCMAKE_BUILD_TYPE=Release` | `juce-eq/build-9/EqProbe_artefacts/Release/EqProbe` |

The same sources compiled for both. No EQ code changes were required. Both probes built in Release.

## Benchmark method

One process at a time. OpenPlugin finishes, then the JUCE 8 probe, then the JUCE 9 probe. The same order is used for the 100-instance run.

- 48 kHz, stereo, blocks of 512.
- Process CPU time (`CLOCK_PROCESS_CPUTIME_ID`), with the buffer-copy loop subtracted.
- Single EQ: 8 seconds of a 1 kHz tone, one warmup pass, median of five.
- 100 EQs: the same 8 seconds, one warmup pass, median of three. Each instance has its own processor and is fed a fresh copy of the block. They are not chained.
- Spectrum feed is off. The display FFT is not in the timed loop.
- DSP modes: minimum phase (`mp`), linear phase (`lp`, 1024-tap FIR, latency 512), Analog ZL (`zl`, same FIR length, latency 0).
- Loads for one EQ: `idle` (six peaks at 0 dB), `one` (one +6 dB bell at 1 kHz), `full` (100 Hz high-pass, 8 kHz low-pass, four +6 dB bells at Q 2).
- The 100-EQ run uses `full` only.

The raw log is `benchmark-juce-8-and-9.log`.

## Results

Run started 10:14:59 CET and finished 10:54:01 CET on 11 November 2026. AI use for that serial benchmark was 39 minutes. Nothing ran in parallel.

Both JUCE probes passed the filter checks before the timing: a +6 dB bell read 6.00 dB in linear phase (latency 512) and 6.03 dB in Analog ZL (latency 0).

### One EQ, percent of realtime

| DSP | Load | OpenPlugin | JUCE 8.0.17-security | JUCE 9.0.5 | Latency |
| --- | --- | ---: | ---: | ---: | ---: |
| Minimum | idle | 0.175 | 0.232 | 0.228 | 0 |
| Minimum | one band | 0.175 | 0.234 | 0.230 | 0 |
| Minimum | full | 0.175 | 0.232 | 0.228 | 0 |
| Linear | idle | 9.07 | 9.28 | 9.28 | 512 |
| Linear | one band | 9.03 | 9.28 | 9.27 | 512 |
| Linear | full | 9.03 | 9.28 | 9.28 | 512 |
| Analog ZL | idle | 9.06 | 9.27 | 9.27 | 0 |
| Analog ZL | one band | 9.06 | 9.27 | 9.29 | 0 |
| Analog ZL | full | 9.04 | 9.29 | 9.27 | 0 |

### One hundred EQs, full patch, percent of one playback

| DSP | OpenPlugin | JUCE 8.0.17-security | JUCE 9.0.5 | Latency each |
| --- | ---: | ---: | ---: | ---: |
| Minimum | 17.5 | 24.6 | 24.1 | 0 |
| Linear | 906 | 937 | 927 | 512 |
| Analog ZL | 891 | 944 | 909 | 0 |

JUCE 8.0.17-security and JUCE 9.0.5 cost the same for this EQ. One hundred minimum-phase instances stay under a quarter of one core. One hundred linear-phase or Analog ZL instances need about nine cores. OpenPlugin stays a little cheaper in every cell. Moving off 8.0.4 did not change that shape.
