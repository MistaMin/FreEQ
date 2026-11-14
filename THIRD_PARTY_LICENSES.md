# Third-party licenses

| Component | Used for | License |
|---|---|---|
| OpenPlugin SDK | Plugin framework and format shells | MIT OR Apache-2.0 |
| GoodLookin DRI | Editor drawing and the spectrum | MIT |
| `cpal` | Standalone audio output | Apache-2.0 |
| `rustfft` | FFT (through the SDK) | MIT OR Apache-2.0 |
| Other Rust crates in `openplugin-eq/Cargo.lock` | Support code | Their own licenses; see each crate |
| VST3 SDK (Steinberg) | VST3 format | Steinberg VST3 SDK license |
| AudioUnitSDK (Apple) | Audio Unit format | Apple Audio Unit SDK license |
| AAX SDK (Avid) | AAX format | Avid AAX Developer Agreement / SDK License |

AAX and Pro Tools are trademarks of Avid Technology, Inc.

The VST3, Audio Unit and AAX license texts are in `Licenses/third-party/` in the installer and the release folder. The AAX SDK itself is not part of this repository.
