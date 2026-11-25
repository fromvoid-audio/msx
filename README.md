# MSX

MSX is a macOS VST3 reference meter that compares your mix against captured peaks across 2–8 frequency bands and passes mono or stereo audio through at unity with zero latency.

- Insert MSX on a bus and configure 2–8 frequency bands and crossovers.
- Capture a reference peak profile from representative audio, then compare it against your mix.
- Spot bands exceeding the reference threshold in orange and adjust Target offset to match your gain staging.

## Build

Requires macOS, Rust, and the [Steinberg VST3 SDK](https://github.com/steinbergmedia/vst3sdk) (`VST3_SDK_DIR`, defaults to `~/lib/vst3sdk`).

```sh
VST3_SDK_DIR=/path/to/vst3sdk cargo build --release --locked  # build optimized library
VST3_SDK_DIR=/path/to/vst3sdk ./scripts/ci.sh                 # check, test, and build
```

Save the project or a host preset to reuse a reference. MSX recalls settings and captures from projects saved with MXH.

## About

I’m [PORTALSURFER](https://soundcloud.com/portalsurfer). I make music and build instruments and tools for making it under [fromvoid](https://github.com/fromvoid-audio). I use AI extensively in that work and share these projects so more people can make music, whatever their budget.

I build this software for myself first and foremost and offer no guarantees of support. That said, I welcome code, documentation, and interesting ideas, and I’m happy to add them when we align on the direction.

---

Copyright wasn't just inconvenienced by AI; perhaps it was always the wrong bet.

We treated ideas like property, then acted surprised when progress slowed and wealth pooled upward. But evolution has always followed a simple pattern: copying, mutation, and selection. Ideas, like living things, develop through variation and iteration.

Perhaps the mistake was never copying, but our insistence on preventing it.

Soon, software will be something we simply wish into existence, endlessly adapting to our needs. When creation becomes abundant, artificial scarcity loses its purpose.

fromvoid is built in the open, not as charity, but as a refusal to restrict the evolution of ideas.

Copy. Mutate. Select. Repeat.

## Support

If you’d like to help cover development costs, you can chip in on Ko-fi:

<a href='https://ko-fi.com/F7X2225YCF' target='_blank'><img height='36' style='border:0px;height:36px;' src='https://storage.ko-fi.com/cdn/kofi6.png?v=6' border='0' alt='Buy Me a Coffee at ko-fi.com' /></a>

---

[Product brief](docs/product-brief.md) · [MIT license](LICENSE)
