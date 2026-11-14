# FreEQ

Six-band equaliser on OpenPlugin. The face and the spectrum are GoodLookin DRI. The crate is `openplugin-eq/`.

Each band is a peak, high-pass, low-pass, low shelf, or high shelf. A click on the band's symbol steps to the next type. MP, LP, and ZL select minimum phase, linear phase, and Analog ZL. Linear phase and Analog ZL play a 1024-tap impulse through `BlockConvolver`. Stereo input and output meters hold their peaks, and a click on the empty plot steps the spectrum between live, frozen, and off.

Version 0.2.0 builds as VST3, Audio Unit, AAX, CLAP, LV2 and a standalone app on macOS. The FreEQ editor is in the standalone app. The plug-in formats show the host's own parameter controls. See [FREEQ.md](FREEQ.md) for the limits of 0.2.0.

```
cargo build --release --manifest-path openplugin-eq/Cargo.toml --bin openplugin-eq
```

That needs the OpenPlugin SDK and GoodLookin checkouts at the paths named in `openplugin-eq/Cargo.toml`.

The report is [FreEQ-Report.pdf](FreEQ-Report.pdf). The editor is in [shots/](shots/).
