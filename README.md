# MSX

<a href='https://ko-fi.com/F7X2225YCF' target='_blank'><img height='36' style='border:0px;height:36px;' src='https://storage.ko-fi.com/cdn/kofi6.png?v=6' border='0' alt='Buy Me a Coffee at ko-fi.com' /></a>

MSX is a macOS VST3 reference meter. It compares your mix against captured peaks across 2–8 frequency bands and passes mono or stereo audio through at unity with zero latency.

1. Insert MSX on a bus that can receive your reference and your mix. Set the bands and crossovers.
2. Click **Capture reference**, play at least one second of representative audio, then click **Use reference**.
3. Switch to your mix. Orange shows where the live level exceeds the reference threshold. Adjust **Target offset** to match your gain staging.

Save the project or a host preset to reuse a reference. MSX recalls settings and reference captures from projects saved with MXH.

Set `VST3_SDK_DIR` and run `./scripts/ci.sh` to check and build the optimized library. Maintainers with the pinned Plugin Release toolkit can also run `./scripts/plugin-release dev --variant msx` to package a local test bundle.

[Product brief](docs/product-brief.md) · [MIT license](LICENSE)
