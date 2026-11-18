# MXH

MXH is a macOS VST3 reference meter built with Toybox. It measures band levels while passing mono or stereo audio through at unity with zero latency.

1. Insert MXH on a bus that can receive your reference track and your mix.
2. Set 2–8 bands and adjust their crossovers. Click the frequency steppers, drag a frequency vertically, or focus it and use the arrow keys.
3. Click **Capture reference**, play a representative section, and click **Use reference**. Capture at least one second of audible material.
4. Switch the bus to your mix. Each band shows the live difference from its target in dB. Orange rises above the target, blue falls below it, and teal marks ±1 dB. Raise a mix element until a band turns orange, then back it down toward teal.
5. Use **Target offset** to align the whole reference with your gain staging. Save the session or a host plug-in preset to reuse the reference.

The captured target is the highest smoothed RMS level reached in each band. Compare passages with similar instrumentation and density. A band without a usable reference, or with audio below the meter's activity gate, shows a dash. **Capture again** retains the prior reference until you accept a replacement. **Clear reference** unlocks the band layout. Tab moves focus, Enter or Space activates buttons, and Shift plus an arrow key makes larger stepper adjustments.

## Build on macOS

Set `VST3_SDK_DIR` to a complete Steinberg VST3 SDK checkout. Run local checks and package an optimized, ad-hoc signed development bundle with:

```sh
./scripts/ci.sh
./scripts/plugin-release dev --variant mxh
```

Copy the resulting `dist/mxh-<build>.vst3` bundle to `~/Library/Audio/Plug-Ins/VST3/MXH.vst3` and rescan VST3 plug-ins in the DAW. The development bundle is for local testing.
