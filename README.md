# FreEQ

Six-band equaliser on OpenPlugin and GoodLookin. The crate is `openplugin-eq/`.

Each band is a peak, high-pass, low-pass, low shelf, or high shelf. MP, LP, and ZL select minimum phase, linear phase, and Analog ZL. Linear phase and Analog ZL play a 1024-tap impulse through `BlockConvolver`.

```
cargo build --release --manifest-path openplugin-eq/Cargo.toml --bin openplugin-eq
```

That needs the OpenPlugin SDK and GoodLookin checkouts at the paths named in `openplugin-eq/Cargo.toml`.

The report is [FreEQ-Report.pdf](FreEQ-Report.pdf). The editor is in [shots/](shots/).
