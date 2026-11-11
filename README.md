# FreEQ

One six-band equaliser, built twice. The Rust realisation is `openplugin-eq/`, on OpenPlugin and GoodLookin. The C++ realisation is `juce-eq/`, compiled against JUCE 8.0.17-security and JUCE 9.0.5 from the same sources.

Each band is a peak, high-pass, low-pass, low shelf, or high shelf. MP, LP, and ZL select minimum phase, linear phase, and Analog ZL. Linear phase and Analog ZL play a 1024-tap impulse with overlap-save. The OpenPlugin path uses `BlockConvolver` from OpenPluginSDK.

The report, with the filters, the measurement, the tables, and the playback code, is [FreEQ-Report.pdf](FreEQ-Report.pdf). The editors are in [shots/](shots/).

`SPEC.md` is the original brief. The equaliser that was measured is the one in the source trees.

JUCE is not included. Point `EQ_JUCE_PATH` at a JUCE checkout. The default in `juce-eq/CMakeLists.txt` is a local JUCE 8 tree.
